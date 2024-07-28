use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use log::{error, info, warn};
use rdev::{listen, Button, Event, EventType, Key};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tauri::{Manager, State, Window};
use tokio::sync::Mutex;
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
            let event_type = match event.event.event_type {
                EventType::KeyPress(Key::Unknown(k)) => format!("KeyPress(Unknown({}))", k),
                EventType::KeyRelease(Key::Unknown(k)) => format!("KeyRelease(Unknown({}))", k),
                _ => format!("{:?}", event.event.event_type),
            };
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

pub struct Recorder {
    session: Arc<Mutex<RecordingSession>>,
    ffmpeg_process: Option<FfmpegChild>,
}

impl Recorder {
    pub fn new() -> Self {
        Recorder {
            session: Arc::new(Mutex::new(RecordingSession::new())),
            ffmpeg_process: None,
        }
    }

    async fn start_recording(
        &mut self,
        window: Window,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session = self.session.clone();

        // Start FFmpeg process
        let output_path = session.lock().await.output_dir.join("recording.mkv");
        self.ffmpeg_process = Some(
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

        // Start frame capture thread
        let frame_session = session.clone();
        tokio::spawn(async move {
            let mut frame_count = 0;
            loop {
                let frame_path = frame_session
                    .lock()
                    .await
                    .output_dir
                    .join(format!("frame_{:05}.jpg", frame_count));
                let timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                // Capture frame using FFmpeg
                if FfmpegCommand::new()
                    .args(&["-f", "avfoundation"])
                    .args(&["-framerate", "1"])
                    .args(&["-i", "1:none"])
                    .args(&["-vframes", "1"])
                    .args(&["-q:v", "2"])
                    .output(frame_path.to_str().unwrap())
                    .spawn()
                    .and_then(|mut child| child.wait())
                    .is_ok()
                {
                    frame_session.lock().await.add_frame(Frame {
                        timestamp,
                        path: frame_path,
                    });
                    frame_count += 1;
                }

                tokio::time::sleep(FRAME_INTERVAL).await;
            }
        });

        // Start event listening thread
        let event_session = session.clone();
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
                }

                let recording_event = RecordingEvent {
                    timestamp,
                    event: event.clone(),
                    mouse_x: last_mouse_pos.0,
                    mouse_y: last_mouse_pos.1,
                };

                let event_session = event_session.clone();
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    event_session.lock().await.add_event(recording_event);
                });
            }) {
                error!("Error: {:?}", error);
            }
        });

        Ok(())
    }

    async fn stop_recording(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(mut process) = self.ffmpeg_process.take() {
            process.kill()?;
            process.wait()?;
        }

        let mut session = self.session.lock().await;
        session.save_events_to_csv().await?;

        info!("Recording saved to {:?}", session.output_dir);

        Ok(())
    }

    async fn analyze_recording(
        &self,
    ) -> Result<RecordingAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        let session = self.session.lock().await;
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

#[derive(Debug, Serialize)]
pub struct RecordingAnalysis {
    total_duration: u64,
    total_events: usize,
    event_counts: HashMap<String, usize>,
    total_mouse_distance: f64,
}

#[tauri::command]
pub async fn start_recording(window: Window, app_handle: tauri::AppHandle) -> Result<(), String> {
    let recorder_state: State<Arc<Mutex<Recorder>>> = app_handle.state();
    let mut recorder = recorder_state.lock().await;
    recorder
        .start_recording(window.clone())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_recording(app_handle: tauri::AppHandle) -> Result<(), String> {
    let recorder_state: State<Arc<Mutex<Recorder>>> = app_handle.state();
    let mut recorder = recorder_state.lock().await;
    recorder.stop_recording().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze_recording(app_handle: tauri::AppHandle) -> Result<RecordingAnalysis, String> {
    let recorder_state: State<Arc<Mutex<Recorder>>> = app_handle.state();
    let recorder = recorder_state.lock().await;
    recorder
        .analyze_recording()
        .await
        .map_err(|e| e.to_string())
}
