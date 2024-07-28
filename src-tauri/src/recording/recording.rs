use chrono::Utc;
use csv::Writer;
use ffmpeg_sidecar::{command::FfmpegCommand, ffprobe::ffprobe_path};
use image::{GenericImageView, Rgba};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut};
use imageproc::rect::Rect;
use log::{debug, error, info, warn};
use rand::seq::SliceRandom;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
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
        let start_time = Instant::now();
        let session_id = Uuid::new_v4();

        info!("Starting recording for session: {}", session_id);
        // ffmpeg -f avfoundation -list_devices true -i ""
        // let _ = FfmpegCommand::new()
        //     .format("avfoundation")
        //     .args(["-list_devices", "true"])
        //     .input("\"\"")
        //     .spawn()
        //     .unwrap()
        //     .iter()
        //     .unwrap()
        //     .for_each(|event: FfmpegEvent| match event {
        //         FfmpegEvent::Log(_level, msg) => {
        //             info!("[ffmpeg] {}", msg);
        //         }
        //         _ => {}
        //     });

        let mut recording_proc = FfmpegCommand::new()
            .format("avfoundation")
            .args(&["-capture_cursor", "1", "-capture_mouse_clicks", "1"])
            .input("1:0")
            .duration(Duration::from_secs(duration).as_secs().to_string())
            .preset("ultrafast")
            .output(self.video_path().as_path().to_str().unwrap())
            .print_command()
            .spawn()
            .unwrap();

        debug!("Spawned in {} seconds", start_time.elapsed().as_millis());

        // Collect cursor data while FFmpeg is recording
        while start_time.elapsed() < Duration::from_secs(duration) {
            if let Ok(pos) = window.cursor_position() {
                let timestamp = Utc::now();

                self.data.push(CursorData {
                    session_id,
                    timestamp: timestamp.timestamp_millis() as u64,
                    x: pos.x as i32,
                    y: pos.y as i32,
                });

                std::thread::sleep(Duration::from_millis(33)); // ~30 FPS
            }
        }

        recording_proc
            .wait()
            .expect("Failed to wait for FFmpeg process");

        info!("Recording complete");
    }

    fn get_video_duration(&self) -> f64 {
        let output = Command::new(ffprobe_path())
            .args(&[
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                self.video_path().to_str().unwrap(),
            ])
            .output()
            .expect("Failed to execute ffprobe");

        info!("FFprobe output: {:?}", &output);

        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<f64>()
            .expect("Failed to parse video duration")
    }

    fn extract_relevant_frames(&mut self) {
        let frames_dir = self.frames_dir();
        fs::create_dir_all(&frames_dir).expect("Failed to create frames directory");

        // Sort the data by timestamp to ensure we process in order
        self.data.sort_by_key(|d| d.timestamp);

        let video_duration_ms = (self.get_video_duration() * 1000.0) as u64;
        let video_path = self.video_path();

        // Get the start timestamp (timestamp of the first frame)
        let start_timestamp = self.data.first().map(|d| d.timestamp).unwrap_or(0);

        for data in &mut self.data {
            // Calculate relative time in milliseconds
            let relative_time_ms = data.timestamp.saturating_sub(start_timestamp);

            if relative_time_ms > video_duration_ms {
                warn!(
                    "Relative time {}ms is beyond video duration, skipping",
                    relative_time_ms
                );
                continue;
            }

            let output_path = frames_dir.join(format!("frame_{}.jpg", data.timestamp));
            let time_str = format!("{}ms", relative_time_ms); // Use exact milliseconds

            let status = FfmpegCommand::new()
                .args(&[
                    "-ss",
                    &time_str,
                    "-i",
                    video_path.to_str().unwrap(),
                    "-vframes",
                    "1",
                    "-q:v",
                    "3",
                    "-pix_fmt",
                    "yuvj420p",
                    output_path.to_str().unwrap(),
                ])
                .print_command()
                .spawn()
                .expect("Failed to execute FFmpeg")
                .wait();

            if status.is_ok() {
                info!(
                    "Extracted frame at relative time {}ms, file: {:?}",
                    relative_time_ms, output_path
                );
            } else {
                error!(
                    "Failed to extract frame at relative time {}ms",
                    relative_time_ms
                );
            }
        }
    }

    fn video_path(&self) -> PathBuf {
        self.session_dir.join("recording.mkv")
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

    fn visualize(&self, num_samples: usize) {
        let mut rng = rand::thread_rng();
        let samples: Vec<&CursorData> = self.data.choose_multiple(&mut rng, num_samples).collect();

        for (i, sample) in samples.iter().enumerate() {
            let full_frame_path = self
                .frames_dir()
                .join(format!("frame_{}.jpg", sample.timestamp));

            if !full_frame_path.exists() {
                info!(
                    "Warning: File does not exist at path: {:?}",
                    full_frame_path
                );
                continue;
            }

            let mut img = image::open(&full_frame_path).expect("Failed to open image");
            let (frame_width, frame_height) = img.dimensions();

            // Ensure cursor coordinates are within image bounds
            let x = sample.x.clamp(0, frame_width as i32 - 1);
            let y = sample.y.clamp(0, frame_height as i32 - 1);

            // Draw a large red rectangle at the cursor position
            draw_filled_rect_mut(
                &mut img,
                Rect::at(x - 10, y - 10).of_size(20, 20),
                Rgba([255, 0, 0, 255]),
            );

            // Draw a yellow circle on top of the rectangle
            draw_filled_circle_mut(&mut img, (x, y), 5, Rgba([255, 255, 0, 255]));

            let output_path = self.session_dir.join(format!("visualized_frame_{}.png", i));
            img.save(&output_path)
                .expect("Failed to save visualized image");

            info!("Visualized frame saved to: {:?}", output_path);
        }
    }
}

#[tauri::command]
pub async fn start_recording(duration: u64, window: tauri::Window) {
    // Spawn the recording task in the background
    task::spawn(async move {
        ffmpeg_sidecar::download::auto_download().unwrap();
        let mut tracker = CursorTracker::new();
        info!("Recording for {} seconds...", duration);
        tracker.start_recording(duration, window.clone());
        info!("Extracting relevant frames...");
        tracker.extract_relevant_frames();
        tracker.save_to_csv();
        info!("Recording saved to {:?}", tracker.session_dir);
        tracker.visualize(5);

        // Emit an event when recording is complete
        window
            .emit("recording_complete", &tracker.session_dir)
            .unwrap();
    });
}

// fn main() {
//     let matches = Command::new("i.inc Desktop Event (devent) Recorder")
//         .version("1.0")
//         .author("Invisibility Inc - Sulaiman Ghori")
//         .about("Tracks cursor movement and captures screenshots")
//         .subcommand(
//             Command::new("record").about("Records cursor movement").arg(
//                 Arg::new("duration")
//                     .short('d')
//                     .long("duration")
//                     .value_name("SECONDS")
//                     .help("Recording duration in seconds")
//                     .default_value("10"),
//             ),
//         )
//         .subcommand(
//             Command::new("visualize")
//                 .about("Visualizes random samples from the latest recording")
//                 .arg(
//                     Arg::new("samples")
//                         .short('n')
//                         .long("samples")
//                         .value_name("NUMBER")
//                         .help("Number of random samples to visualize")
//                         .default_value("5"),
//                 ),
//         )
//         .get_matches();

//     match matches.subcommand() {
//         Some(("record", record_matches)) => {
//             let duration = record_matches
//                 .get_one::<String>("duration")
//                 .unwrap()
//                 .parse()
//                 .expect("Invalid duration");

//             let mut tracker = CursorTracker::new();
//             println!("Recording for {} seconds...", duration);
//             tracker.start_recording(duration);
//             println!("Extracting relevant frames...");
//             tracker.extract_relevant_frames();
//             tracker.save_to_csv();
//             println!("Recording saved to {:?}", tracker.session_dir);
//         }
//         Some(("visualize", visualize_matches)) => {
//             let samples = visualize_matches
//                 .get_one::<String>("samples")
//                 .unwrap()
//                 .parse()
//                 .expect("Invalid number of samples");

//             // Find the latest recording directory
//             let latest_dir = fs::read_dir("output")
//                 .expect("Failed to read output directory")
//                 .filter_map(|entry| {
//                     let entry = entry.ok()?;
//                     let path = entry.path();
//                     if path.is_dir() {
//                         Some(path)
//                     } else {
//                         None
//                     }
//                 })
//                 .max_by_key(|path| path.metadata().unwrap().modified().unwrap())
//                 .expect("No recording found");

//             // Load the data from the CSV file
//             let mut tracker = CursorTracker {
//                 session_dir: latest_dir,
//                 data: Vec::new(),
//             };
//             let csv_path = tracker.devent_path();
//             let mut reader = Reader::from_path(csv_path).expect("Failed to read CSV");

//             for result in reader.records() {
//                 let record = result.expect("Failed to read CSV record");
//                 tracker.data.push(CursorData {
//                     timestamp: record[0].to_string(),
//                     x: record[1].parse().unwrap(),
//                     y: record[2].parse().unwrap(),
//                     frame_path: Some(record[3].to_string()),
//                 });
//             }

//             println!("Visualizing {} random samples...", samples);
//             tracker.visualize(samples);
//         }
//         _ => println!("Please specify a valid subcommand. Use --help for more information."),
//     }
// }
