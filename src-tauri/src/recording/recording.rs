use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use image::{GenericImageView, Rgba};
use imageproc::drawing::draw_filled_rect_mut;
use imageproc::rect::Rect;
use log::{debug, error, info, warn};
use rdev::{listen, Event, EventType};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
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

    fn frames_dir(&self) -> PathBuf {
        self.output_dir.join("frames")
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

    fn read_timestamps(&self) -> Vec<u64> {
        let file = File::open(self.timestamp_path()).expect("Failed to open timestamp file");
        let reader = BufReader::new(file);

        reader
            .lines()
            .skip(1) // Skip the header line
            .filter_map(|line| line.ok()?.parse::<u64>().ok())
            .collect()
    }

    pub fn extract_frames_around_events(
        &mut self,
        frames_before: usize,
        frames_after: usize,
    ) -> Result<()> {
        let frames_dir = self.frames_dir();
        fs::create_dir_all(&frames_dir)?;
        let video_path = self.video_path();

        // Read timestamps from the file
        let video_timestamps = self.read_timestamps();

        for (event_index, event) in self.events.iter().enumerate() {
            // Find the closest video timestamp to the event
            let closest_timestamp_index = video_timestamps
                .binary_search_by(|probe| probe.cmp(&event.timestamp))
                .unwrap_or_else(|x| x);

            let start_frame = closest_timestamp_index.saturating_sub(frames_before);
            let end_frame =
                (closest_timestamp_index + frames_after + 1).min(video_timestamps.len());

            for frame_index in start_frame..end_frame {
                let frame_timestamp = video_timestamps[frame_index];
                let output_path = frames_dir.join(format!(
                    "event_{:04}_frame_{:04}_{}.jpg",
                    event_index,
                    frame_index - start_frame,
                    frame_timestamp
                ));

                // Extract frame
                let status = FfmpegCommand::new()
                    .args(&["-i", video_path.to_str().unwrap()])
                    .args(&["-vf", &format!("select='eq(n,{})'", frame_index)])
                    .args(&["-vframes", "1"])
                    .args(&["-q:v", "3"])
                    .args(&["-pix_fmt", "yuvj420p"])
                    .output(output_path.to_str().unwrap())
                    .spawn()?
                    .wait()?;

                if status.success() {
                    info!(
                        "Extracted frame for event {} at timestamp {}, file: {:?}",
                        event_index, frame_timestamp, output_path
                    );

                    // Visualize cursor position on the frame
                    self.visualize_cursor_on_frame(&output_path, event)?;
                } else {
                    error!(
                        "Failed to extract frame at timestamp {} for event {}",
                        frame_timestamp, event_index
                    );
                }
            }
        }

        info!("Extracted frames around {} events", self.events.len());
        Ok(())
    }

    fn visualize_cursor_on_frame(
        &self,
        frame_path: &PathBuf,
        event: &RecordingEvent,
    ) -> Result<()> {
        let mut img = image::open(frame_path)?;
        let (width, height) = img.dimensions();

        // Ensure cursor coordinates are within image bounds
        let x = event.mouse_x.clamp(0.0, width as f64 - 1.0) as i32;
        let y = event.mouse_y.clamp(0.0, height as f64 - 1.0) as i32;

        // Draw a red box around the cursor position
        draw_filled_rect_mut(
            &mut img,
            Rect::at(x - 10, y - 10).of_size(20, 20),
            Rgba([255, 0, 0, 128]), // Semi-transparent red
        );

        // Add text to describe the event
        // let event_description = format!("{:?}", event.event.event_type);
        // imageproc::drawing::draw_text_mut(
        //     &mut img,
        //     Rgba([255, 255, 255, 255]),
        //     x + 15,
        //     y + 15,
        //     rusttype::Scale::uniform(20.0),
        //     &rusttype::Font::try_from_bytes(include_bytes!("../assets/DejaVuSans.ttf")).unwrap(),
        //     &event_description,
        // );
        info!(
            "{:?} at x: {}, y: {} at timestamp {}",
            event.event.event_type, x, y, event.timestamp
        );

        img.save(frame_path)?;
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

        info!("Stopping recording");

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

        Ok(())
    }

    fn analyze_recording(&self) {
        let mut session = self.session.lock().unwrap();
        if let Some(session) = session.as_mut() {
            match session.extract_frames_around_events(5, 5) {
                Ok(_) => info!("Frame extraction and event visualization completed"),
                Err(e) => error!("Error during frame extraction: {:?}", e),
            }
        } else {
            error!("No active recording session");
        }
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
        // NOTE: gotta grab scaling factor of monitor
        if let EventType::MouseMove { x, y } = event.event_type {
            last_mouse_pos = (x * 2 as f64, y * 2 as f64);
            // Do not record MouseMove events
            return;
        }

        // NOTE: drag halts mousemove so we need to update last_mouse_pos here, not a nice way to
        // do that with rdev so we can use window or something

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

#[tauri::command]
pub fn start_recording(app_handle: AppHandle, state: State<'_, RecorderState>) {
    _ = state.start_recording(app_handle);
}

#[tauri::command]
pub fn stop_recording(app_handle: AppHandle, state: State<'_, RecorderState>) {
    _ = state.stop_recording(app_handle);
    state.analyze_recording();
}
