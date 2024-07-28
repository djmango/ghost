use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::command::FfmpegCommand;
use image::{GenericImageView, Rgba};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut};
use imageproc::rect::Rect;
use log::{debug, error, info, warn};
use rand::seq::SliceRandom;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use tauri::{Emitter, Window};
use tokio::task;
use uuid::Uuid;

#[derive(Debug)]
struct CursorTracker {
    /// Directory to store the recording session
    session_dir: PathBuf,
    /// Cursor data collected during the recording
    data: Vec<CursorData>,
}

#[derive(Debug, Clone)]
struct CursorData {
    /// UUID for the recording session
    session_id: Uuid,
    /// Timestamp in milliseconds
    timestamp: u64,
    /// X coordinate of the cursor
    x: i32,
    /// Y coordinate of the cursor
    y: i32,
}

impl CursorTracker {
    fn new() -> Self {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let session_dir = PathBuf::from(format!("output/{}", timestamp));
        fs::create_dir_all(&session_dir).expect("Failed to create session directory");

        CursorTracker {
            session_dir,
            data: Vec::new(),
        }
    }

    fn start_recording(&mut self, duration: u64, window: Window) {
        let session_id = Uuid::new_v4();
        info!("Starting recording for session: {}", session_id);

        let video_path = self.video_path();
        let timestamp_path = self.timestamp_path();

        let mut recording_proc = FfmpegCommand::new()
            .args(&["-f", "avfoundation"]) // Use avfoundation for macOS
            .args(&["-capture_cursor", "1"])
            .args(&["-capture_mouse_clicks", "1"])
            .duration(Duration::from_secs(duration).as_secs().to_string())
            .args(&["-framerate", "30"])
            .args(&["-i", "1:none"]) // Adjust this for your specific input device
            .args(&[
                "-filter_complex",
                "settb=1/1000,setpts='RTCTIME/1000-1500000000000',mpdecimate,split=2[out][ts]",
            ])
            .args(&["-map", "[out]"])
            .args(&["-vcodec", "libx264"])
            .args(&["-pix_fmt", "yuv420p"])
            .args(&["-preset", "fast"])
            .args(&["-crf", "0"])
            .args(&["-threads", "0"])
            .output(video_path.to_str().unwrap())
            .args(&["-map", "[ts]"])
            .args(&["-f", "mkvtimestamp_v2"])
            .output(timestamp_path.to_str().unwrap())
            .args(&["-vsync", "0"])
            .print_command()
            .spawn()
            .unwrap();

        // Start collecting cursor data
        let start_time = Instant::now();
        while start_time.elapsed() < Duration::from_secs(duration) {
            if let Ok(pos) = window.cursor_position() {
                let current_timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                self.data.push(CursorData {
                    session_id,
                    timestamp: current_timestamp,
                    x: pos.x as i32,
                    y: pos.y as i32,
                });

                std::thread::sleep(Duration::from_millis(10)); // ~100Hz
            }
        }

        recording_proc
            .wait()
            .expect("Failed to wait for FFmpeg process");
        info!("Recording complete");
    }

    fn extract_relevant_frames(&mut self) {
        let frames_dir = self.frames_dir();
        fs::create_dir_all(&frames_dir).expect("Failed to create frames directory");

        let video_path = self.video_path();
        let timestamp_path = self.timestamp_path();

        // Read timestamps from the file
        let timestamps = self.read_timestamps(timestamp_path);

        for (index, data) in self.data.iter().enumerate() {
            if index >= timestamps.len() {
                warn!("More cursor data than video frames, stopping extraction");
                break;
            }

            let frame_timestamp = timestamps[index] + 1500000000000; // Add back the subtracted value
            let output_path = frames_dir.join(format!("frame_{}.jpg", frame_timestamp));

            let status = FfmpegCommand::new()
                .args(&["-i", video_path.to_str().unwrap()])
                .args(&["-vf", &format!("select='eq(n,{})'", index)])
                .args(&["-vframes", "1"])
                .args(&["-q:v", "3"])
                .args(&["-pix_fmt", "yuvj420p"])
                .output(output_path.to_str().unwrap())
                .print_command()
                .spawn()
                .expect("Failed to execute FFmpeg")
                .wait();

            if status.is_ok() {
                info!(
                    "Extracted frame at timestamp {}, file: {:?}",
                    frame_timestamp, output_path
                );
            } else {
                error!("Failed to extract frame at timestamp {}", frame_timestamp);
            }
        }
    }

    fn read_timestamps(&self, path: PathBuf) -> Vec<u64> {
        let file = File::open(path).expect("Failed to open timestamp file");
        let reader = BufReader::new(file);

        reader
            .lines()
            .skip(1) // Skip the header line
            .filter_map(|line| line.ok()?.parse::<u64>().ok())
            .collect()
    }
    fn create_visualization_video(&self) {
        let frames_dir = self.frames_dir();
        let temp_dir = self.session_dir.join("temp_frames");
        std::fs::create_dir_all(&temp_dir).expect("Failed to create temp frames directory");

        // Load a system font

        for (index, cursor_data) in self.data.iter().enumerate() {
            let frame_path = frames_dir.join(format!("frame_{}.jpg", cursor_data.timestamp));
            let mut img = image::open(&frame_path).expect("Failed to open image");

            // Draw cursor position
            draw_filled_circle_mut(
                &mut img,
                (cursor_data.x, cursor_data.y),
                5,
                Rgba([255, 0, 0, 255]),
            );

            // Draw timestamp
            // let text = format!("Frame: {}, Timestamp: {}", index, cursor_data.timestamp);
            // let size = 20.0;
            // draw_text_mut(
            //     &mut img,
            //     Rgba([255, 255, 255, 255]),
            //     10,
            //     10,
            //     PxScale::from(size),
            //     &font.into(),
            //     &text,
            // );

            // Save the modified frame
            let temp_frame_path = temp_dir.join(format!("frame_{:05}.jpg", index));
            img.save(&temp_frame_path).expect("Failed to save frame");
        }
    }

    fn video_path(&self) -> PathBuf {
        self.session_dir.join("recording.mkv")
    }

    fn timestamp_path(&self) -> PathBuf {
        self.session_dir.join("timestamps.txt")
    }

    fn frames_dir(&self) -> PathBuf {
        self.session_dir.join("frames")
    }

    fn devent_path(&self) -> PathBuf {
        self.session_dir.join("devents.csv")
    }

    fn save_to_csv(&self) {
        let csv_path = self.devent_path();
        let mut writer = Writer::from_path(csv_path).expect("Failed to create CSV writer");
        writer
            .write_record(&["session_id", "timestamp", "x", "y"])
            .expect("Failed to write CSV header");

        for data in &self.data {
            writer
                .write_record(&[
                    &data.session_id.to_string(),
                    &data.timestamp.to_string(),
                    &data.x.to_string(),
                    &data.y.to_string(),
                ])
                .expect("Failed to write CSV record");
        }

        writer.flush().expect("Failed to flush CSV writer");
    }

    fn align_cursor_data_with_frames(&mut self) {
        let frames_dir = self.frames_dir();

        // Get all frame timestamps
        let mut frame_timestamps: Vec<u64> = fs::read_dir(&frames_dir)
            .expect("Failed to read frames directory")
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_file() && path.extension()? == "jpg" {
                    let filename = path.file_name()?.to_str()?;
                    let timestamp = filename.strip_prefix("frame_")?.strip_suffix(".jpg")?;
                    timestamp.parse::<u64>().ok()
                } else {
                    None
                }
            })
            .collect();

        frame_timestamps.sort_unstable();

        // Sort cursor data by timestamp
        self.data.sort_by_key(|d| d.timestamp);

        let mut aligned_data = Vec::new();
        let mut cursor_index = 0;

        for (frame_index, &frame_timestamp) in frame_timestamps.iter().enumerate() {
            let mut best_diff = u64::MAX;
            let mut best_cursor_data = None;

            // Find the closest cursor data point
            while cursor_index < self.data.len() {
                let cursor_data = &self.data[cursor_index];
                let diff = (frame_timestamp as i64 - cursor_data.timestamp as i64).abs() as u64;

                if diff < best_diff {
                    best_diff = diff;
                    best_cursor_data = Some(cursor_data.clone());
                    cursor_index += 1;
                } else if cursor_data.timestamp > frame_timestamp {
                    // We've gone too far, break out of the loop
                    break;
                } else {
                    cursor_index += 1;
                }
            }

            if let Some(mut cursor_data) = best_cursor_data {
                cursor_data.timestamp = frame_timestamp;
                aligned_data.push(cursor_data.clone());

                debug!(
                    "Frame {}: timestamp {}, aligned with cursor timestamp {}, diff: {}ms",
                    frame_index, frame_timestamp, cursor_data.timestamp, best_diff
                );
            } else {
                warn!(
                    "No suitable cursor data found for frame timestamp: {}",
                    frame_timestamp
                );
            }
        }

        info!(
            "Aligned {} cursor data points with {} frames",
            aligned_data.len(),
            frame_timestamps.len()
        );

        if aligned_data.len() < frame_timestamps.len() {
            warn!(
                "Some frames ({}) don't have corresponding cursor data",
                frame_timestamps.len() - aligned_data.len()
            );
        }

        self.data = aligned_data;
    }

    fn visualize(&self, num_samples: usize) {
        let frames_dir = self.frames_dir();

        let mut rng = rand::thread_rng();
        let samples = self
            .data
            .choose_multiple(&mut rng, num_samples.min(self.data.len()));

        for (i, cursor_data) in samples.enumerate() {
            let frame_path = frames_dir.join(format!("frame_{}.jpg", cursor_data.timestamp));

            if frame_path.exists() {
                let mut img = image::open(&frame_path).expect("Failed to open image");
                let (frame_width, frame_height) = img.dimensions();

                // Ensure cursor coordinates are within image bounds
                let x = cursor_data.x.clamp(0, frame_width as i32 - 1);
                let y = cursor_data.y.clamp(0, frame_height as i32 - 1);

                // Draw a large red rectangle at the cursor position
                draw_filled_rect_mut(
                    &mut img,
                    Rect::at(x - 10, y - 10).of_size(20, 20),
                    Rgba([255, 0, 0, 255]),
                );

                // Draw a yellow circle on top of the rectangle
                draw_filled_circle_mut(&mut img, (x, y), 5, Rgba([255, 255, 0, 255]));

                let output_path = self.session_dir.join(format!("visualized_frame_{}.jpg", i));
                img.save(&output_path)
                    .expect("Failed to save visualized image");

                info!("Visualized frame saved to: {:?}", output_path);
            } else {
                warn!("Frame file not found: {:?}", frame_path);
            }
        }
    }
}

impl CursorTracker {
    fn extract_frames_and_align_cursor_data(
        &mut self,
        params: AlignmentParams,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let frames_dir = self.frames_dir();
        fs::create_dir_all(&frames_dir)?;
        let video_path = self.video_path();
        let timestamp_path = self.timestamp_path();

        // Read timestamps from the file
        let timestamps = self.read_timestamps(timestamp_path);

        let mut aligned_data = Vec::new();
        let mut frame_count = 0;

        for (index, cursor_data) in self.data.iter().enumerate() {
            if index >= timestamps.len() {
                warn!("More cursor data than video frames, stopping extraction");
                break;
            }

            let frame_timestamp = timestamps[index] + 1500000000000; // Add back the subtracted value
            let adjusted_timestamp = frame_timestamp as i64 + params.time_offset;
            let output_path = frames_dir.join(format!("frame_{}.jpg", adjusted_timestamp));

            // Extract frame
            let status = FfmpegCommand::new()
                .args(&["-i", video_path.to_str().unwrap()])
                .args(&["-vf", &format!("select='eq(n,{})'", index)])
                .args(&["-vframes", "1"])
                .args(&["-q:v", "3"])
                .args(&["-pix_fmt", "yuvj420p"])
                .output(output_path.to_str().unwrap())
                .print_command()
                .spawn()?
                .wait()?;

            if status.success() {
                info!(
                    "Extracted frame at timestamp {}, file: {:?}",
                    adjusted_timestamp, output_path
                );
                frame_count += 1;

                // Find closest cursor data
                let closest_cursor_data = self.find_closest_cursor_data(
                    adjusted_timestamp as u64,
                    params.match_direction,
                    params.tolerance,
                );

                if let Some(mut matched_cursor_data) = closest_cursor_data {
                    let time_diff =
                        adjusted_timestamp as i64 - matched_cursor_data.timestamp as i64;
                    debug!(
                        "Frame {}: timestamp {}, matched with cursor timestamp {}, diff: {}ms",
                        index, adjusted_timestamp, matched_cursor_data.timestamp, time_diff
                    );

                    matched_cursor_data.timestamp = adjusted_timestamp as u64;
                    aligned_data.push(matched_cursor_data);
                } else {
                    warn!(
                        "No suitable cursor data found for frame timestamp: {}",
                        adjusted_timestamp
                    );
                }
            } else {
                error!(
                    "Failed to extract frame at timestamp {}",
                    adjusted_timestamp
                );
            }
        }

        info!(
            "Extracted {} frames and aligned {} cursor data points",
            frame_count,
            aligned_data.len()
        );

        if aligned_data.len() < frame_count {
            warn!(
                "{} frames don't have corresponding cursor data",
                frame_count - aligned_data.len()
            );
        }

        self.data = aligned_data;

        self.print_alignment_statistics();

        Ok(())
    }

    fn find_closest_cursor_data(
        &self,
        target_timestamp: u64,
        direction: MatchDirection,
        tolerance: u64,
    ) -> Option<CursorData> {
        let mut closest_data = None;
        let mut min_diff = u64::MAX;

        for data in &self.data {
            let diff = match direction {
                MatchDirection::Forward => target_timestamp.saturating_sub(data.timestamp),
                MatchDirection::Backward => data.timestamp.saturating_sub(target_timestamp),
                MatchDirection::Bidirectional => {
                    (target_timestamp as i64 - data.timestamp as i64).abs() as u64
                }
            };

            if diff < min_diff && diff <= tolerance {
                min_diff = diff;
                closest_data = Some(data.clone());
            }

            if direction == MatchDirection::Forward && data.timestamp > target_timestamp {
                break;
            }
        }

        closest_data
    }

    fn print_alignment_statistics(&self) {
        let mut total_diff = 0i64;
        let mut max_diff = 0i64;
        let mut min_diff = i64::MAX;

        for (i, data) in self.data.iter().enumerate() {
            let frame_timestamp = data.timestamp;
            let cursor_timestamp = self.data[i].timestamp;
            let diff = frame_timestamp as i64 - cursor_timestamp as i64;

            total_diff += diff;
            max_diff = max_diff.max(diff);
            min_diff = min_diff.min(diff);

            debug!(
                "Frame {}: Frame timestamp {}, Cursor timestamp {}, Difference: {}ms",
                i, frame_timestamp, cursor_timestamp, diff
            );
        }

        let avg_diff = if !self.data.is_empty() {
            total_diff as f64 / self.data.len() as f64
        } else {
            0.0
        };

        info!("Alignment Statistics:");
        info!("  Total frames: {}", self.data.len());
        info!("  Average difference: {:.2}ms", avg_diff);
        info!("  Maximum difference: {}ms", max_diff);
        info!("  Minimum difference: {}ms", min_diff);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchDirection {
    Forward,
    Backward,
    Bidirectional,
}

pub struct AlignmentParams {
    pub time_offset: i64,
    pub tolerance: u64,
    pub match_direction: MatchDirection,
}

#[tauri::command]
pub async fn start_recording(duration: u64, window: tauri::Window) {
    ffmpeg_sidecar::download::auto_download().unwrap();
    let mut tracker = CursorTracker::new();

    info!("Recording for {} seconds...", duration);
    let start_time = std::time::Instant::now();
    tracker.start_recording(duration, window.clone());
    let elapsed = start_time.elapsed();
    info!("Recording completed in {:?}", elapsed);

    info!(
        "Number of cursor data points collected: {}",
        tracker.data.len()
    );
    if let Some(first) = tracker.data.first() {
        if let Some(last) = tracker.data.last() {
            info!(
                "Cursor data time range: {} to {} ({} ms)",
                first.timestamp,
                last.timestamp,
                last.timestamp - first.timestamp
            );
        }
    }

    info!("Extracting relevant frames...");
    // tracker.extract_relevant_frames();

    // info!("Aligning cursor data with frames...");
    // tracker.align_cursor_data_with_frames();

    let params = AlignmentParams {
        time_offset: -50, // Adjust if needed
        tolerance: 100,   // 100ms tolerance
        match_direction: MatchDirection::Forward,
    };

    _ = tracker.extract_frames_and_align_cursor_data(params);

    tracker.save_to_csv();
    info!("Recording saved to {:?}", tracker.session_dir);
    // tracker.visualize(5);

    info!("Creating visualization video...");
    tracker.create_visualization_video();
    info!("Visualization video created");

    window
        .emit("recording_complete", &tracker.session_dir)
        .unwrap();
}
