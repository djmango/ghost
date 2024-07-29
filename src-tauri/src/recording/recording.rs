use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use log::{debug, error, info, warn};
use rdev::{listen, Event, EventType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordingEvent {
    timestamp: u64,
    event: Event,
    mouse_x: f64,
    mouse_y: f64,
}

#[derive(Debug)]
struct RecordingSession {
    id: Uuid,
    start_time: Instant,
    events: Vec<RecordingEvent>,
    output_dir: PathBuf,
}

impl RecordingSession {
    fn new() -> Result<Self> {
        let id = Uuid::new_v4();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let output_dir = PathBuf::from(format!("output/{}", timestamp));
        fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

        Ok(RecordingSession {
            id,
            start_time: Instant::now(),
            events: Vec::new(),
            output_dir,
        })
    }

    fn video_path(&self) -> PathBuf {
        self.output_dir.join("recording.mkv")
    }

    fn timestamp_path(&self) -> PathBuf {
        self.output_dir.join("timestamps.txt")
    }

    fn csv_path(&self) -> PathBuf {
        self.output_dir.join("events.csv")
    }

    fn save_events_to_csv(&self) -> Result<()> {
        let mut writer = Writer::from_path(self.csv_path())?;
        writer.write_record(&["timestamp", "event_type", "details", "mouse_x", "mouse_y"])?;

        for event in &self.events {
            let event_type = format!("{:?}", event.event.event_type);
            let details = match event.event.event_type {
                EventType::KeyPress(key) | EventType::KeyRelease(key) => format!("{:?}", key),
                EventType::ButtonPress(button) | EventType::ButtonRelease(button) => {
                    format!("{:?}", button)
                }
                EventType::MouseMove { x, y } => format!("x: {}, y: {}", x, y),
                EventType::Wheel { delta_x, delta_y } => {
                    format!("delta_x: {}, delta_y: {}", delta_x, delta_y)
                }
            };
            writer.write_record(&[
                &event.timestamp.to_string(),
                &event_type,
                &details,
                &event.mouse_x.to_string(),
                &event.mouse_y.to_string(),
            ])?;
        }

        writer.flush()?;
        Ok(())
    }
}

pub struct RecorderState {
    session: Arc<Mutex<Option<RecordingSession>>>,
    ffmpeg_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    ffmpeg_child: Arc<Mutex<Option<FfmpegChild>>>,
    event_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    is_recording: Arc<AtomicBool>,
}

impl RecorderState {
    pub fn new() -> Self {
        RecorderState {
            session: Arc::new(Mutex::new(None)),
            ffmpeg_handle: Arc::new(Mutex::new(None)),
            ffmpeg_child: Arc::new(Mutex::new(None)),
            event_handle: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(AtomicBool::new(false)),
        }
    }

    fn start_recording(&self, app_handle: AppHandle) -> Result<()> {
        let mut session_guard = self.session.lock().unwrap();
        if session_guard.is_some() {
            return Err(anyhow!("Recording is already in progress"));
        }
        let new_session = RecordingSession::new()?;
        let output_path = new_session.video_path();
        let timestamp_path = new_session.timestamp_path();
        *session_guard = Some(new_session);
        drop(session_guard);

        self.is_recording.store(true, Ordering::SeqCst);

        // Start FFmpeg process in a separate thread
        let is_recording = self.is_recording.clone();
        let ffmpeg_child = self.ffmpeg_child.clone();
        let ffmpeg_handle = thread::spawn(move || {
            let child = FfmpegCommand::new()
                .args(&["-f", "avfoundation"])
                .args(&["-capture_cursor", "1"])
                .args(&["-capture_mouse_clicks", "1"])
                .args(&["-framerate", "30"])
                .args(&["-i", "1:none"])
                .args(&["-vcodec", "libx264"])
                .args(&["-preset", "ultrafast"])
                .args(&["-crf", "23"])
                .args(&[
                    "-filter_complex",
                    "settb=1/1000,setpts='RTCTIME/1000',mpdecimate,split=2[out][ts]",
                ])
                .args(&["-map", "[out]"])
                .args(&["-vcodec", "libx264"])
                .args(&["-pix_fmt", "yuv420p"])
                .args(&["-threads", "0"])
                .output(output_path.to_str().unwrap())
                .args(&["-map", "[ts]"])
                .args(&["-f", "mkvtimestamp_v2"])
                .output(timestamp_path.to_str().unwrap())
                .args(&["-vsync", "0"])
                .spawn()
                .expect("Failed to start FFmpeg");

            *ffmpeg_child.lock().unwrap() = Some(child);

            while is_recording.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }

            // Gracefully stop FFmpeg
            if let Some(mut child) = ffmpeg_child.lock().unwrap().take() {
                info!("Stopping FFmpeg");
                match child.quit() {
                    Ok(_) => {
                        match child.wait() {
                            Ok(exit_status) => info!("FFmpeg stopped with {:?}", exit_status),
                            Err(e) => {
                                error!("Failed to stop FFmpeg: {:?}", e);
                                warn!("Force killing FFmpeg");
                                // If still running, force kill
                                let _ = child.kill();
                                let _ = child.wait();
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to stop FFmpeg: {:?}", e);
                        warn!("Force killing FFmpeg");
                        // If still running, force kill
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            }
        });

        *self.ffmpeg_handle.lock().unwrap() = Some(ffmpeg_handle);

        // Start event capture in a separate thread
        let session = self.session.clone();
        let is_recording = self.is_recording.clone();
        let event_handle = thread::spawn(move || {
            let _ = event_capture_task(session, is_recording);
        });

        *self.event_handle.lock().unwrap() = Some(event_handle);

        info!("Recording started successfully");
        app_handle.emit("recording_started", ()).unwrap();

        Ok(())
    }

    fn stop_recording(&self, app_handle: AppHandle) -> Result<()> {
        // Signal threads to stop
        self.is_recording.store(false, Ordering::SeqCst);

        // Wait for FFmpeg thread to finish
        if let Some(handle) = self.ffmpeg_handle.lock().unwrap().take() {
            handle.join().unwrap();
        }

        // Wait for event capture thread to finish
        if let Some(handle) = self.event_handle.lock().unwrap().take() {
            handle.join().unwrap();
        }

        debug!("Stopping recording");

        // Save events to CSV
        let mut session_guard = self.session.lock().unwrap();
        if let Some(s) = session_guard.as_mut() {
            s.save_events_to_csv()?;
            info!("Recording saved to {:?}", s.output_dir);
            app_handle
                .emit("recording_complete", s.output_dir.to_str())
                .unwrap();
        } else {
            return Err(anyhow!("No active recording session"));
        }

        *session_guard = None;

        Ok(())
    }
    fn analyze_recording(&self) -> Result<RecordingAnalysis> {
        let session_guard = self.session.lock().unwrap();
        let session = session_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No recording session available"))?;

        let total_duration = session.start_time.elapsed();

        let mut event_counts = HashMap::new();
        let mut total_mouse_distance = 0.0;
        let mut last_mouse_pos = None;

        for event in &session.events {
            let event_type = format!("{:?}", event.event.event_type);
            *event_counts.entry(event_type).or_insert(0) += 1;

            // Calculate mouse movement
            if let Some((prev_x, prev_y)) = last_mouse_pos {
                let dx = event.mouse_x - prev_x;
                let dy = event.mouse_y - prev_y;
                total_mouse_distance += (dx as f64 * dx as f64 + dy as f64 * dy as f64).sqrt();
            }
            last_mouse_pos = Some((event.mouse_x, event.mouse_y));
        }

        Ok(RecordingAnalysis {
            total_duration: total_duration.as_secs(),
            total_events: session.events.len(),
            event_counts,
            total_mouse_distance,
        })
    }
}

fn event_capture_task(
    session: Arc<Mutex<Option<RecordingSession>>>,
    is_recording: Arc<AtomicBool>,
) -> Result<()> {
    let mut last_mouse_pos = (0.0, 0.0);
    let _ = listen(move |event| {
        if !is_recording.load(Ordering::SeqCst) {
            return;
        }

        let timestamp = Utc::now().timestamp_millis() as u64;

        // Update last known mouse position if this is a mouse move event
        if let EventType::MouseMove { x, y } = event.event_type {
            last_mouse_pos = (x, y);
            // Do not record MouseMove events
            return;
        }

        let recording_event = RecordingEvent {
            timestamp,
            event: event.clone(),
            mouse_x: last_mouse_pos.0,
            mouse_y: last_mouse_pos.1,
        };

        info!("{:?}", recording_event);

        let mut session_guard = session.lock().unwrap();
        if let Some(s) = session_guard.as_mut() {
            s.events.push(recording_event);
        }
    })
    .map_err(|e| anyhow!("Event capture failed: {:?}", e));

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct RecordingAnalysis {
    total_duration: u64,
    total_events: usize,
    event_counts: HashMap<String, usize>,
    total_mouse_distance: f64,
}

#[tauri::command]
pub fn start_recording(app_handle: AppHandle, state: State<'_, RecorderState>) {
    state.start_recording(app_handle);
}

#[tauri::command]
pub fn stop_recording(app_handle: AppHandle, state: State<'_, RecorderState>) {
    state.stop_recording(app_handle);
}

#[tauri::command]
pub fn analyze_recording(state: State<'_, RecorderState>) -> Result<RecordingAnalysis, String> {
    state.analyze_recording().map_err(|e| {
        warn!("Failed to analyze recording: {:?}", e);
        e.to_string()
    })
}
