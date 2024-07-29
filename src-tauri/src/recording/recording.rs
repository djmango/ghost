use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use log::{debug, error, info, warn};
use rdev::{listen, Event, EventType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tauri::Emitter;
use tauri::{AppHandle, Manager, State, Window};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

// Constants
const FRAME_BUFFER_SIZE: usize = 30; // Number of frames to keep before and after an event
const FRAME_INTERVAL: Duration = Duration::from_millis(33); // ~30 fps

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordingEvent {
    timestamp: u64,
    event: Event,
    mouse_x: f64,
    mouse_y: f64,
}

#[derive(Debug)]
struct Frame {
    timestamp: u64,
    path: PathBuf,
}

#[derive(Debug)]
struct RecordingSession {
    id: Uuid,
    start_time: SystemTime,
    events: Vec<RecordingEvent>,
    frames: VecDeque<Frame>,
    output_dir: PathBuf,
}

impl RecordingSession {
    fn new() -> Self {
        let id = Uuid::new_v4();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let output_dir = PathBuf::from(format!("output/{}", timestamp));
        fs::create_dir_all(&output_dir).expect("Failed to create output directory");

        RecordingSession {
            id,
            start_time: SystemTime::now(),
            events: Vec::new(),
            frames: VecDeque::with_capacity(FRAME_BUFFER_SIZE * 2),
            output_dir,
        }
    }

    fn add_event(&mut self, event: RecordingEvent) {
        self.events.push(event);
    }

    fn add_frame(&mut self, frame: Frame) {
        if self.frames.len() >= FRAME_BUFFER_SIZE * 2 {
            if let Some(old_frame) = self.frames.pop_front() {
                fs::remove_file(old_frame.path).unwrap_or_else(|e| {
                    warn!("Failed to remove old frame: {:?}", e);
                });
            }
        }
        self.frames.push_back(frame);
    }

    async fn save_events_to_csv(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let csv_path = self.output_dir.join("events.csv");
        let mut writer = Writer::from_path(csv_path)?;
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
    session: Arc<RwLock<Option<RecordingSession>>>,
    ffmpeg_process: Arc<Mutex<Option<FfmpegChild>>>,
    frame_capture_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    event_capture_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl RecorderState {
    pub fn new() -> Self {
        RecorderState {
            session: Arc::new(RwLock::new(None)),
            ffmpeg_process: Arc::new(Mutex::new(None)),
            frame_capture_handle: Arc::new(Mutex::new(None)),
            event_capture_handle: Arc::new(Mutex::new(None)),
        }
    }

    async fn start_recording(
        &self,
        app_handle: AppHandle,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut session_guard = self.session.write().await;
        if session_guard.is_some() {
            return Err("Recording is already in progress".into());
        }
        *session_guard = Some(RecordingSession::new());
        let session_clone = self.session.clone();
        drop(session_guard); // Release the write lock

        // Start FFmpeg process
        let output_path = {
            let session = self.session.read().await;
            session.as_ref().unwrap().output_dir.join("recording.mkv")
        };
        let mut ffmpeg_process = self.ffmpeg_process.lock().await;
        *ffmpeg_process = Some(
            FfmpegCommand::new()
                .args(&["-f", "avfoundation"])
                .args(&["-capture_cursor", "1"])
                .args(&["-capture_mouse_clicks", "1"])
                .args(&["-framerate", "30"])
                .args(&["-i", "1:none"])
                .args(&["-vcodec", "libx264"])
                .args(&["-preset", "ultrafast"])
                .args(&["-crf", "23"])
                .output(output_path.to_str().unwrap())
                .spawn()?,
        );

        // Start frame capture task
        let frame_capture_handle = tokio::spawn(frame_capture_task(session_clone.clone()));
        let mut frame_handle = self.frame_capture_handle.lock().await;
        *frame_handle = Some(frame_capture_handle);

        // Start event capture task
        let event_capture_handle = tokio::spawn(event_capture_task(session_clone));
        let mut event_handle = self.event_capture_handle.lock().await;
        *event_handle = Some(event_capture_handle);

        info!("Recording started successfully");
        app_handle
            .emit("recording_started", ())
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn stop_recording(
        &self,
        app_handle: AppHandle,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Stop FFmpeg process
        let mut ffmpeg_process = self.ffmpeg_process.lock().await;
        if let Some(mut process) = ffmpeg_process.take() {
            process.kill()?;
            process.wait()?;
        }

        debug!("Stopping recording");

        // Stop frame capture task
        debug!("Stopping frame capture task");
        let mut frame_handle = self.frame_capture_handle.lock().await;
        if let Some(handle) = frame_handle.take() {
            handle.abort();
        }
        debug!("Frame capture task stopped");

        // Stop event capture task
        debug!("Stopping event capture task");
        let mut event_handle = self.event_capture_handle.lock().await;
        if let Some(handle) = event_handle.take() {
            handle.abort();
        }
        debug!("Event capture task stopped");

        // Save events to CSV
        let mut session_guard = self.session.write().await;
        if let Some(s) = session_guard.as_mut() {
            s.save_events_to_csv().await?;
            info!("Recording saved to {:?}", s.output_dir);
            app_handle
                .emit("recording_complete", s.output_dir.to_str())
                .map_err(|e| e.to_string())?;
        } else {
            return Err("No active recording session".into());
        }

        *session_guard = None;

        Ok(())
    }

    async fn analyze_recording(
        &self,
    ) -> Result<RecordingAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        let session_guard = self.session.read().await;
        let session = session_guard
            .as_ref()
            .ok_or("No recording session available")?;

        let total_duration = SystemTime::now().duration_since(session.start_time)?;

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

async fn frame_capture_task(session: Arc<RwLock<Option<RecordingSession>>>) {
    let mut frame_count = 0;
    loop {
        tokio::time::sleep(FRAME_INTERVAL).await;

        let session_guard = session.read().await;
        if session_guard.is_none() {
            break;
        }
        let session_ref = session_guard.as_ref().unwrap();

        let frame_path = session_ref
            .output_dir
            .join(format!("frame_{:05}.jpg", frame_count));
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Capture frame using FFmpeg
        if FfmpegCommand::new()
            .args(&["-f", "avfoundation"])
            .args(&["-capture_cursor", "1"])
            .args(&["-capture_mouse_clicks", "1"])
            .args(&["-framerate", "1"])
            .args(&["-i", "1:none"])
            .args(&["-vframes", "1"])
            .args(&["-q:v", "2"])
            .output(frame_path.to_str().unwrap())
            .spawn()
            .and_then(|mut child| child.wait())
            .is_ok()
        {
            drop(session_guard); // Release the read lock before acquiring the write lock
            let mut session_guard = session.write().await;
            if let Some(s) = session_guard.as_mut() {
                s.add_frame(Frame {
                    timestamp,
                    path: frame_path,
                });
                frame_count += 1;
            }
        }
    }
}

async fn event_capture_task(session: Arc<RwLock<Option<RecordingSession>>>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    // Spawn a thread for event listening
    std::thread::spawn(move || {
        let mut last_mouse_pos = (0.0, 0.0);
        if let Err(error) = listen(move |event| {
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            // Update last known mouse position if this is a mouse move event
            if let EventType::MouseMove { x, y } = event.event_type {
                last_mouse_pos = (x, y);
                info!("Mouse position: ({}, {})", x, y);
                // Do not emit MouseMove events
                return;
            }

            // Drag events are not handled properly

            let recording_event = RecordingEvent {
                timestamp,
                event: event.clone(),
                mouse_x: last_mouse_pos.0,
                mouse_y: last_mouse_pos.1,
            };

            info!("{:?}", recording_event);

            // let _ = tx.send(recording_event);
            let _ = tx.blocking_send(recording_event);
        }) {
            error!("Error in event listener: {:?}", error);
        }
    });

    // Process events in the async task
    while let Some(event) = rx.recv().await {
        let mut session_guard = session.write().await;
        if let Some(s) = session_guard.as_mut() {
            s.add_event(event);
        } else {
            break;
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RecordingAnalysis {
    total_duration: u64,
    total_events: usize,
    event_counts: HashMap<String, usize>,
    total_mouse_distance: f64,
}

#[tauri::command]
pub async fn start_recording(
    app_handle: AppHandle,
    state: State<'_, RecorderState>,
) -> Result<(), String> {
    state
        .start_recording(app_handle)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_recording(
    app_handle: AppHandle,
    state: State<'_, RecorderState>,
) -> Result<(), String> {
    state
        .stop_recording(app_handle)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze_recording(
    state: State<'_, RecorderState>,
) -> Result<RecordingAnalysis, String> {
    state.analyze_recording().await.map_err(|e| e.to_string())
}
