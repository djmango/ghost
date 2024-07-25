use chrono::Utc;
use clap::{Arg, Command};
use csv::{Reader, Writer};
use mouse_rs::Mouse;
use rand::seq::SliceRandom;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{Duration, Instant};

struct CursorTracker {
    session_dir: PathBuf,
    video_path: PathBuf,
    data: Vec<CursorData>,
}

struct CursorData {
    timestamp: String,
    x: i32,
    y: i32,
    frame_path: Option<String>,
}

impl CursorTracker {
    fn new() -> Self {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let session_dir = PathBuf::from(format!("output/{}", timestamp));
        fs::create_dir_all(&session_dir).expect("Failed to create session directory");
        let video_path = session_dir.join("recording.mkv");
        CursorTracker {
            session_dir,
            video_path,
            data: Vec::new(),
        }
    }

    fn start_recording(&mut self, duration: u64) {
        let start_time = Instant::now();
        let mouse = Mouse::new();

        // Start FFmpeg process
        let mut ffmpeg = ProcessCommand::new("ffmpeg")
            .args(&[
                "-f",
                "avfoundation",
                "-capture_cursor",
                "1",
                "-capture_mouse_clicks",
                "1",
                "-i",
                "1:0",
                "-t",
                &duration.to_string(),
                self.video_path.to_str().unwrap(),
            ])
            .spawn()
            .expect("Failed to start FFmpeg");

        // Collect cursor data while FFmpeg is recording
        while start_time.elapsed() < Duration::from_secs(duration) {
            let pos = mouse.get_position().expect("Failed to get mouse position");
            let timestamp = Utc::now().format("%Y%m%d%H%M%S%.3f").to_string();

            self.data.push(CursorData {
                timestamp,
                x: pos.x,
                y: pos.y,
                frame_path: None,
            });

            std::thread::sleep(Duration::from_millis(33)); // ~30 FPS
        }

        // Wait for FFmpeg to finish
        ffmpeg.wait().expect("FFmpeg failed");
    }

    fn extract_relevant_frames(&mut self) {
        let frames_dir = self.session_dir.join("frames");
        fs::create_dir_all(&frames_dir).expect("Failed to create frames directory");

        // Extract all frames at once
        let status = ProcessCommand::new("ffmpeg")
            .args(&[
                "-i",
                self.video_path.to_str().unwrap(),
                "-vf",
                "fps=30",
                "-q:v",
                "2", // High quality (lower means higher quality)
                "-pix_fmt",
                "rgb24", // Force RGB24 pixel format
                frames_dir.join("frame_%05d.png").to_str().unwrap(),
            ])
            .status()
            .expect("Failed to extract frames");

        if !status.success() {
            println!("Warning: Frame extraction may have failed");
        }

        // Update CursorData with frame paths
        for (i, data) in self.data.iter_mut().enumerate() {
            let frame_path = format!("frames/frame_{:05}.png", i + 1);
            if frames_dir.join(&frame_path).exists() {
                data.frame_path = Some(frame_path);
            } else {
                println!("Warning: Frame not found for timestamp: {}", data.timestamp);
            }
        }
    }

    fn save_to_csv(&self) {
        let csv_path = self.session_dir.join("cursor_data.csv");
        let mut writer = Writer::from_path(csv_path).expect("Failed to create CSV writer");
        writer
            .write_record(&["timestamp", "x", "y", "frame_path"])
            .expect("Failed to write CSV header");

        for data in &self.data {
            writer
                .write_record(&[
                    &data.timestamp,
                    &data.x.to_string(),
                    &data.y.to_string(),
                    data.frame_path.as_deref().unwrap_or(""),
                ])
                .expect("Failed to write CSV record");
        }

        writer.flush().expect("Failed to flush CSV writer");
    }

    fn visualize(&self, num_samples: usize) {
        let mut rng = rand::thread_rng();
        let samples: Vec<&CursorData> = self.data.choose_multiple(&mut rng, num_samples).collect();

        for (i, sample) in samples.iter().enumerate() {
            if let Some(frame_path) = &sample.frame_path {
                let img =
                    image::open(self.session_dir.join(frame_path)).expect("Failed to open image");
                let mut img = img.to_rgb8();

                // Draw a red circle at the cursor position
                imageproc::drawing::draw_filled_circle_mut(
                    &mut img,
                    (sample.x, sample.y),
                    5,
                    image::Rgb([255, 0, 0]),
                );

                let output_path = self.session_dir.join(format!("visualized_frame_{}.png", i));
                img.save(&output_path)
                    .expect("Failed to save visualized image");

                println!("Visualized frame saved to: {:?}", output_path);
            }
        }
    }
}

fn main() {
    let matches = Command::new("i.inc Desktop Event (devent) Recorder")
        .version("1.0")
        .author("Invisibility Inc - Sulaiman Ghori")
        .about("Tracks cursor movement and captures screenshots")
        .subcommand(
            Command::new("record").about("Records cursor movement").arg(
                Arg::new("duration")
                    .short('d')
                    .long("duration")
                    .value_name("SECONDS")
                    .help("Recording duration in seconds")
                    .default_value("10"),
            ),
        )
        .subcommand(
            Command::new("visualize")
                .about("Visualizes random samples from the latest recording")
                .arg(
                    Arg::new("samples")
                        .short('n')
                        .long("samples")
                        .value_name("NUMBER")
                        .help("Number of random samples to visualize")
                        .default_value("5"),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("record", record_matches)) => {
            let duration = record_matches
                .get_one::<String>("duration")
                .unwrap()
                .parse()
                .expect("Invalid duration");

            let mut tracker = CursorTracker::new();
            println!("Recording for {} seconds...", duration);
            tracker.start_recording(duration);
            println!("Extracting relevant frames...");
            tracker.extract_relevant_frames();
            tracker.save_to_csv();
            println!("Recording saved to {:?}", tracker.session_dir);
        }
        Some(("visualize", visualize_matches)) => {
            let samples = visualize_matches
                .get_one::<String>("samples")
                .unwrap()
                .parse()
                .expect("Invalid number of samples");

            // Find the latest recording directory
            let latest_dir = fs::read_dir("output")
                .expect("Failed to read output directory")
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    if path.is_dir() {
                        Some(path)
                    } else {
                        None
                    }
                })
                .max_by_key(|path| path.metadata().unwrap().modified().unwrap())
                .expect("No recording found");

            // Load the data from the CSV file
            let csv_path = latest_dir.join("cursor_data.csv");
            let mut reader = Reader::from_path(csv_path).expect("Failed to read CSV");
            let mut tracker = CursorTracker {
                session_dir: latest_dir,
                video_path: PathBuf::new(), // We don't need this for visualization
                data: Vec::new(),
            };

            for result in reader.records() {
                let record = result.expect("Failed to read CSV record");
                tracker.data.push(CursorData {
                    timestamp: record[0].to_string(),
                    x: record[1].parse().unwrap(),
                    y: record[2].parse().unwrap(),
                    frame_path: Some(record[3].to_string()),
                });
            }

            println!("Visualizing {} random samples...", samples);
            tracker.visualize(samples);
        }
        _ => println!("Please specify a valid subcommand. Use --help for more information."),
    }
}
