use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::{ffmpeg_is_installed, FfmpegCommand};
use ffmpeg_sidecar::download::auto_download;
use ffmpeg_sidecar::event::FfmpegEvent;
use log::{debug, error, info, warn};
use rdev::{listen, Event, EventType};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::async_runtime::TokioRuntime;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
//use tauri::window::Window;
use tauri::{Manager, WebviewWindow};

use crate::types::{KeyboardAction, KeyboardActionKey, MouseAction, ScrollAction};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordingEvent {
    timestamp: u64,
    event: Event,
    mouse_x: f64,
    mouse_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeventRequest {
    pub session_id: Uuid,
    pub mouse_action: Option<MouseAction>,
    pub keyboard_action: Option<KeyboardAction>,
    pub scroll_action: Option<ScrollAction>,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub event_timestamp_nanos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveRecordingRequest {
    pub recording_id: Uuid,
    pub session_id: Uuid,
    pub start_timestamp_nanos: i64,
    pub duration_ms: u64,
}

#[derive(Debug)]
struct RecordingSession {
    id: Uuid,
    events: Vec<RecordingEvent>,
    output_dir: PathBuf,
}

impl RecordingSession {
    fn new(app_handle: Arc<AppHandle>) -> Result<Self> {
        // Store the recording session in a unique directory under app data (different but
        // predictable per OS, we should always have read/write access)
        let id = Uuid::new_v4();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let output_dir = app_handle
            .path()
            .app_data_dir()
            .unwrap()
            .join(format!("output/{}", timestamp));
        fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

        let recordings_dir = output_dir.join("recordings");
        fs::create_dir_all(&recordings_dir).context("Failed to create recordings directory")?;

        Ok(RecordingSession {
            // app_handle,
            id,
            events: Vec::new(),
            output_dir,
        })
    }

    fn video_path(&self) -> PathBuf {
        self.output_dir.join("recordings")
    }

    fn segment_csv_path(&self) -> PathBuf {
        self.output_dir.join("segments.csv")
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

fn get_ffmpeg_command(
    video_output_path: &str,
    segment_csv_path: &str,
    timestamp_path: &str,
) -> FfmpegCommand {
    let mut cmd = FfmpegCommand::new();

    // OS-specific input configuration
    #[cfg(target_os = "macos")]
    {
        cmd.args(["-f", "avfoundation"])
            .args(["-capture_cursor", "1"])
            // .args(["-capture_mouse_clicks", "1"])
            .args(["-i", "2:none"]);
    }

    #[cfg(target_os = "windows")]
    {
        cmd.args(["-f", "gdigrab"])
            .args(["-capture_cursor", "1"])
            .args(["-i", "desktop"]);
    }

    #[cfg(target_os = "linux")]
    {
        cmd.args(["-f", "x11grab"])
            .args(["-capture_cursor", "1"])
            .args(["-i", ":0.0"]);
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
        .args(["-vcodec", "libx264"])
        .args(["-pix_fmt", "yuv420p"])
        .args(["-threads", "0"])
        .args(["-force_key_frames", "expr:gte(t,n_forced*60)"])
        .args(["-f", "segment"])
        .args(["-segment_time", "60"]) // 60 seconds per chunk
        .args(["-reset_timestamps", "1"])
        .args(["-segment_format", "mkv"])
        .args(["-segment_list_type", "csv"])
        .args(["-segment_list", segment_csv_path])
        .output(video_output_path)
        .args(["-map", "[ts]"])
        .args(["-f", "mkvtimestamp_v2"])
        .arg(timestamp_path)
        .args(["-vsync", "0"]);

    debug!("COMMAND: {:?}", cmd);
    debug!("OUTPUT: {:?}", video_output_path);

    list_ffmpeg_devices();

    cmd
}

fn list_ffmpeg_devices() {
    FfmpegCommand::new()
        .args(&["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .spawn()
        .expect("Failed to spawn FFmpeg")
        .iter()
        .expect("Failed to get output")
        .for_each(|line| match line {
            FfmpegEvent::Log(_, line) => debug!("[ffmpeg log] {}", line),
            _ => {}
        });
}

pub struct RecorderState {
    app_handle: Arc<AppHandle>,
    event_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    ffmpeg_child: Arc<Mutex<Option<FfmpegChild>>>,
    ffmpeg_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    is_recording: Arc<AtomicBool>,
    runtime: Arc<TokioRuntime>,
    session: Arc<Mutex<Option<RecordingSession>>>,
}

impl RecorderState {
    pub fn new(app_handle: &AppHandle) -> Self {
        RecorderState {
            app_handle: Arc::new(app_handle.clone()),
            event_handle: Arc::new(Mutex::new(None)),
            ffmpeg_child: Arc::new(Mutex::new(None)),
            ffmpeg_handle: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(AtomicBool::new(false)),
            runtime: Arc::new(TokioRuntime::new().expect("Failed to create Tokio runtime")),
            session: Arc::new(Mutex::new(None)),
        }
    }

    fn start_recording(&self) -> Result<()> {
        let mut session_guard = self.session.lock().unwrap();
        if session_guard.is_some() {
            return Err(anyhow!("Recording is already in progress"));
        }
        let new_session = RecordingSession::new(self.app_handle.clone())?;
        let session_id = new_session.id;
        let video_dir_path = new_session.video_path();
        let video_dir_path_clone = video_dir_path.clone();
        let timestamp_path = new_session.timestamp_path();
        let segment_csv_path = new_session.segment_csv_path();
        let segment_csv_path_clone = segment_csv_path.clone();
        *session_guard = Some(new_session);
        drop(session_guard);

        self.is_recording.store(true, Ordering::SeqCst);

        // Start FFmpeg process in a separate thread
        let is_recording = self.is_recording.clone();
        let ffmpeg_child = self.ffmpeg_child.clone();

        let ffmpeg_handle = thread::spawn({
            let is_recording = is_recording.clone();
            let ffmpeg_child = ffmpeg_child.clone();

            move || {
                let child_result = get_ffmpeg_command(
                    video_dir_path.join("chunk_%04d.mkv").to_str().unwrap(),
                    segment_csv_path.to_str().unwrap(),
                    timestamp_path.to_str().unwrap(),
                )
                .spawn();
                // .expect("Failed to start FFmpeg");

                let mut child = match child_result {
                    Ok(child) => child,
                    Err(e) => {
                        error!("Failed to start FFmpeg: {}", e);
                        error!("Error kind: {:?}", e.kind());
                        error!("Raw OS error: {:?}", e.raw_os_error());

                        // If it's a "not found" error, it might be a PATH issue
                        if e.kind() == std::io::ErrorKind::NotFound {
                            error!("FFmpeg command not found. Check if FFmpeg is installed and in PATH.");
                            // You might want to print the current PATH for debugging
                            if let Ok(path) = std::env::var("PATH") {
                                error!("Current PATH: {}", path);
                            }
                        }

                        panic!("Cannot proceed without FFmpeg");
                    }
                };

                let stdout = child.take_stdout().expect("Failed to get stdout");
                let stderr = child.take_stderr().expect("Failed to get stderr");

                *ffmpeg_child.lock().expect("Failed to lock ffmpeg_child") = Some(child);

                let stdout_handle = thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            debug!("[ffmpeg stdout] {}", line);
                        }
                    }
                });

                let stderr_handle = thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            debug!("[ffmpeg stderr] {}", line);
                        }
                    }
                });

                info!("Recording started and waiting for stop signal");
                while is_recording.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(100));
                }

                // Gracefully stop FFmpeg
                if let Some(mut child) = ffmpeg_child
                    .lock()
                    .expect("Failed to lock ffmpeg_child")
                    .take()
                {
                    info!("Stopping FFmpeg");
                    match child.quit() {
                        Ok(_) => {
                            match child.wait() {
                                Ok(exit_status) => info!("FFmpeg stopped with {:?}", exit_status),
                                Err(e) => {
                                    error!("Failed to stop FFmpeg: {:?}", e);
                                    warn!("Force killing FFmpeg");
                                    // If still running, force kill
                                    _ = child.kill();
                                    _ = child.wait();
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to stop FFmpeg: {:?}", e);
                            warn!("Force killing FFmpeg");
                            // If still running, force kill
                            _ = child.kill();
                            _ = child.wait();
                        }
                    }
                }

                // Wait for stdout and stderr threads to finish
                stdout_handle.join().expect("Failed to join stdout thread");
                stderr_handle.join().expect("Failed to join stderr thread");
            }
        });

        *self
            .ffmpeg_handle
            .lock()
            .expect("Failed to lock ffmpeg_handle") = Some(ffmpeg_handle);

        // Start event capture in a separate thread
        let session = self.session.clone();
        let is_recording = self.is_recording.clone();
        let main_window = self
            .app_handle
            .get_webview_window("main")
            .expect("Failed to get main window");
        let runtime = self.runtime.clone();
        let runtime_clone = runtime.clone();
        let event_handle = thread::spawn(move || {
            event_capture_task(session, is_recording, main_window, runtime)
                .expect("Failed to start event capture");
        });
        thread::spawn(move || {
            monitor_segments(
                video_dir_path_clone,
                segment_csv_path_clone,
                session_id,
                runtime_clone,
            );
        });

        *self
            .event_handle
            .lock()
            .expect("Failed to lock event_handle") = Some(event_handle);

        info!("Recording started successfully");
        self.app_handle
            .emit("recording_started", ())
            .expect("Failed to emit event");

        Ok(())
    }

    async fn stop_recording(&self) -> Result<()> {
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
            info!("Recording saved to {:?}", s.output_dir.canonicalize()?);
            self.app_handle
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
    runtime: Arc<TokioRuntime>,
) -> Result<()> {
    let mut last_mouse_pos = (0.0, 0.0);
    let _ = listen(move |event| {
        if !is_recording.load(Ordering::SeqCst) {
            return;
        }

        // Get current time in nanoseconds
        let timestamp = match Utc::now().timestamp_nanos_opt() {
            Some(timestamp) => timestamp as u64,
            None => return, // After the year 2262 this always be the case
        };

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

        // info!("{:?}", recording_event);

        let mut session_guard = session.lock().unwrap();
        if let Some(s) = session_guard.as_mut() {
            s.events.push(recording_event);

            let mut create_devent_request = CreateDeventRequest {
                session_id: s.id,
                mouse_action: None,
                keyboard_action: None,
                scroll_action: None,
                mouse_x: last_mouse_pos.0 as i32,
                mouse_y: last_mouse_pos.1 as i32,
                event_timestamp_nanos: timestamp as i64,
            };

            match event.event_type {
                EventType::ButtonPress(btn) => {
                    let mouse_action: MouseAction = btn.into();
                    create_devent_request.mouse_action = Some(mouse_action);
                }
                EventType::KeyPress(key) => {
                    let keyboard_action: KeyboardActionKey = key.into();
                    create_devent_request.keyboard_action = Some(KeyboardAction {
                        key: keyboard_action,
                        duration: 100, // TODO: make this dynamic by tracking keypress and keyrelease events
                    });
                }
                EventType::Wheel { delta_x, delta_y } => {
                    let scroll_action: ScrollAction = ScrollAction {
                        x: delta_x as i32,
                        y: delta_y as i32,
                    };
                    create_devent_request.scroll_action = Some(scroll_action);
                }
                _ => return,
            };

            let runtime = runtime.clone();
            runtime.spawn(async move {
                let client = reqwest::Client::new();
                let res = client
                    .post("https://echo.i.inc/devents/create")
                    .json(&create_devent_request)
                    .send()
                    .await;

                match res {
                    Ok(_) => info!("Event saved successfully"),
                    Err(e) => error!("Failed to send request: {:?}", e),
                }
            });
        }
    })
    .map_err(|e| anyhow!("Event capture failed: {:?}", e));

    Ok(())
}

fn monitor_segments(
    recording_dir_path: PathBuf,
    segment_csv_path: PathBuf,
    session_id: Uuid,
    runtime: Arc<TokioRuntime>,
) {
    let mut last_position = 0;

    loop {
        thread::sleep(Duration::from_secs(5));
        let file = File::open(&segment_csv_path).unwrap();
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(last_position)).unwrap();

        let mut buffer = String::new();
        loop {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => break, // End of file
                Ok(_) => {
                    // Process the new segment
                    let parts: Vec<&str> = buffer.trim().split(',').collect();
                    if parts.len() == 3 {
                        let filename = parts[0].to_string();
                        let start_time: f64 = parts[1].parse().unwrap_or_default();
                        let end_time: f64 = parts[2].parse().unwrap_or_default();
                        info!("New segment: {}, {} to {}", filename, start_time, end_time);

                        let client = reqwest::Client::new();
                        let recording_dir_path_clone = recording_dir_path.clone();

                        runtime.spawn(async move {
                            let res = client
                                .post("https://echo.i.inc/recordings/fetch_save_url")
                                .json(&SaveRecordingRequest {
                                    recording_id: Uuid::new_v4(),
                                    session_id,
                                    start_timestamp_nanos: start_time as i64,
                                    duration_ms: (end_time - start_time) as u64,
                                })
                                .send()
                                .await;

                            match res {
                                Ok(res) => {
                                    let url = res.text().await.unwrap();

                                    let video_file_path = recording_dir_path_clone.join(filename);
                                    let mut video_file = File::open(&video_file_path).unwrap();
                                    let mut video_content = Vec::new();
                                    video_file.read_to_end(&mut video_content).unwrap();

                                    let upload_res = client
                                        .put(url)
                                        .header("Content-Type", "video/x-matroska")
                                        .body(video_content)
                                        .send()
                                        .await;

                                    match upload_res {
                                        Ok(_) => info!("Uploaded recording successfully"),
                                        Err(e) => error!("Failed to upload recording: {:?}", e),
                                    }
                                }
                                Err(e) => error!("Failed to send request: {:?}", e),
                            }
                        });
                    }

                    // Update last_position after processing each line
                    match reader.stream_position() {
                        Ok(pos) => last_position = pos,
                        Err(e) => {
                            error!("Failed to get stream position: {:?}", e);
                            break;
                        }
                    };
                }
                Err(e) => {
                    error!("Error reading line: {:?}", e);
                    break;
                }
            }
        }
    }
}

#[tauri::command]
pub fn start_recording(state: State<'_, RecorderState>) {
    info!("Ffmpeg installed: {:?}", ffmpeg_is_installed());
    auto_download().expect("Failed to download ffmpeg");

    _ = state.start_recording();
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, RecorderState>) -> Result<(), String> {
    state.stop_recording().await.map_err(|e| e.to_string())?;
    Ok(())
}
