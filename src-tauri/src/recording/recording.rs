use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use log::{error, info, warn};
use rdev::{listen, Event, EventType};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
//use tauri::window::Window;
use tauri::{Manager, WebviewWindow};

use tauri_plugin_store::StoreCollection;
use serde_json::Value;

struct AppState {
    username: Mutex<String>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            username: Mutex::new(String::new()),
        }
    }
}

#[tauri::command]
fn get_email(app_handle: tauri::AppHandle) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let stores = app_handle.state::<StoreCollection<tauri::Wry>>();
    let path: PathBuf = PathBuf::from("store.bin");

    tauri_plugin_store::with_store(app_handle.clone(), stores, path, |store| {
        let email = store.get("email")
            .and_then(|v| v.as_str().map(String::from));
        
        Ok(email)
    }).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

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

    fn csv_path(&self) -> PathBuf {
        self.output_dir.join("events.csv")
    }

    fn timestamp_path(&self) -> PathBuf {
        self.output_dir.join("timestamps.txt")
    }

    fn save_events_to_csv(&self) -> Result<()> {
        let mut writer = Writer::from_path(self.csv_path())?;
        writer.write_record(["timestamp", "event_type", "details", "mouse_x", "mouse_y"])?;

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
            writer.write_record([
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

fn get_ffmpeg_command(output_path: &str, timestamp_path: &str) -> FfmpegCommand {
    let mut cmd = FfmpegCommand::new();

    // OS-specific input configuration
    #[cfg(target_os = "macos")]
    {
        cmd.args(["-f", "avfoundation"])
            .args(["-capture_cursor", "1"])
            .args(["-capture_mouse_clicks", "1"])
            .args(["-i", "1:none"]);
    }

    #[cfg(target_os = "windows")]
    {
        cmd.args(["-f", "gdigrab"]).args(["-i", "desktop"]);
    }

    #[cfg(target_os = "linux")]
    {
        cmd.args(["-f", "x11grab"]).args(["-i", ":0.0"]);
    }

    // Common configuration for all platforms
    cmd.args(["-framerate", "30"])
        .args(["-vcodec", "libx264"])
        .args(["-preset", "ultrafast"])
        .args(["-crf", "23"])
        .args([
            "-filter_complex",
            "settb=1/1000,setpts='RTCTIME/1000',mpdecimate,split=2[out][ts]",
        ])
        .args(["-map", "[out]"])
        .args(["-pix_fmt", "yuv420p"])
        .args(["-threads", "0"])
        .args(["-y"])  // Overwrite output file if it exists
        .arg("-o")  // Explicitly specify output file
        .arg(output_path)
        .args(["-map", "[ts]"])
        .args(["-f", "mkvtimestamp_v2"])
        .arg(timestamp_path)
        .args(["-vsync", "0"]);

    cmd
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
        // Get the username
        let username = get_email(app_handle.clone())
            .unwrap_or(None)
            .unwrap_or_else(|| "Unknown User".to_string());
    
        // Log the username
        info!("Starting recording for user: {}", username);
    
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
            let mut cmd = get_ffmpeg_command(
                output_path.to_str().unwrap(),
                timestamp_path.to_str().unwrap(),
            );
            let child = cmd.spawn().expect("Failed to start FFmpeg");
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
        let main_window = app_handle
            .get_webview_window("main")
            .expect("Failed to get main window");
        let event_handle = thread::spawn(move || {
            let _ = event_capture_task(session, is_recording, main_window);
        });

        *self.event_handle.lock().unwrap() = Some(event_handle);

        info!("Recording started successfully");
        app_handle.emit("recording_started", ()).unwrap();

        Ok(())
    }

    async fn stop_recording(&self, app_handle: AppHandle) -> Result<()> {
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
}

fn event_capture_task(
    session: Arc<Mutex<Option<RecordingSession>>>,
    is_recording: Arc<AtomicBool>,
    main_window: WebviewWindow,
) -> Result<()> {
    let mut last_mouse_pos = (0.0, 0.0);
    let _ = listen(move |event| {
        if !is_recording.load(Ordering::SeqCst) {
            return;
        }

        let timestamp = Utc::now().timestamp_millis() as u64;

        // Get the scale factor
        let scale_factor = main_window.scale_factor().unwrap_or(1.0);

        // Update last known mouse position if this is a mouse move event
        if let EventType::MouseMove { x, y } = event.event_type {
            last_mouse_pos = (x * scale_factor, y * scale_factor);
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
pub async fn stop_recording(
    app_handle: AppHandle<tauri::Wry>,
    state: State<'_, RecorderState>,
) -> Result<(), String> {
    state
        .stop_recording(app_handle)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

