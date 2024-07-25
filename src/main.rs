use chrono::Utc;
use clap::{Arg, Command};
use csv::Writer;
use mouse_rs::Mouse;
use std::fs;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};
use std::time::{Duration, Instant};

struct CursorTracker {
    output_dir: PathBuf,
    video_path: PathBuf,
    data: Vec<CursorData>,
}

struct CursorData {
    timestamp: String,
    x: i32,
    y: i32,
}

impl CursorTracker {
    fn new(output_dir: PathBuf) -> Self {
        fs::create_dir_all(&output_dir).expect("Failed to create output directory");
        let video_path = output_dir.join("recording.mkv");
        CursorTracker {
            output_dir,
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
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
            });

            std::thread::sleep(Duration::from_millis(33)); // ~30 FPS
        }

        // Wait for FFmpeg to finish
        ffmpeg.wait().expect("FFmpeg failed");
    }

    fn save_to_csv(&self, csv_file: &PathBuf) {
        let mut writer = Writer::from_path(csv_file).expect("Failed to create CSV writer");
        writer
            .write_record(&["timestamp", "x", "y"])
            .expect("Failed to write CSV header");

        for data in &self.data {
            writer
                .write_record(&[&data.timestamp, &data.x.to_string(), &data.y.to_string()])
                .expect("Failed to write CSV record");
        }

        writer.flush().expect("Failed to flush CSV writer");
    }

    fn display_random(&self, dataset: Option<&str>) {
        // Implementation for display_random
        println!("Display random functionality not implemented yet");
        if let Some(dataset_name) = dataset {
            println!("Using dataset: {}", dataset_name);
        }
    }

    fn upload_to_hf(&self, dataset_name: &str) {
        // Implementation for upload_to_hf
        println!("Upload to Hugging Face functionality not implemented yet");
        println!("Dataset name: {}", dataset_name);
    }
}
fn main() {
    let matches = Command::new("i.inc Desktop Event (devent) Recorder")
        .version("1.0")
        .author("Invisibility Inc - Sulaiman Ghori")
        .about("Tracks cursor movement and captures screenshots")
        .subcommand(
            Command::new("record")
                .about("Records cursor movement")
                .arg(
                    Arg::new("duration")
                        .short('d')
                        .long("duration")
                        .value_name("SECONDS")
                        .help("Recording duration in seconds")
                        .default_value("10"),
                )
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("DIR")
                        .help("Output directory for recordings")
                        .default_value("output"),
                ),
        )
        .subcommand(
            Command::new("display")
                .about("Displays a random screenshot")
                .arg(
                    Arg::new("dataset")
                        .long("dataset")
                        .value_name("DATASET")
                        .help("Specific dataset to use"),
                ),
        )
        .subcommand(
            Command::new("upload")
                .about("Uploads data to Hugging Face")
                .arg(
                    Arg::new("dataset")
                        .long("dataset")
                        .value_name("DATASET")
                        .help("Local dataset to upload")
                        .required(true),
                )
                .arg(
                    Arg::new("hf-dataset")
                        .long("hf-dataset")
                        .value_name("HF_DATASET")
                        .help("Hugging Face dataset name")
                        .required(true),
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
            let output_dir = PathBuf::from(record_matches.get_one::<String>("output").unwrap());
            let mut tracker = CursorTracker::new(output_dir.clone());
            println!("Recording for {} seconds...", duration);
            tracker.start_recording(duration);
            let csv_file = output_dir.join(format!("recording_{}.csv", Utc::now().timestamp()));
            tracker.save_to_csv(&csv_file);
            println!("Recording saved to {}", csv_file.display());
        }
        Some(("display", display_matches)) => {
            let output_dir = PathBuf::from("output"); // You might want to make this configurable
            let tracker = CursorTracker::new(output_dir);
            tracker.display_random(
                display_matches
                    .get_one::<String>("dataset")
                    .map(|s| s.as_str()),
            );
        }
        Some(("upload", upload_matches)) => {
            let output_dir = PathBuf::from("output"); // You might want to make this configurable
            let tracker = CursorTracker::new(output_dir);
            let dataset = upload_matches.get_one::<String>("dataset").unwrap();
            let hf_dataset = upload_matches.get_one::<String>("hf-dataset").unwrap();
            tracker.upload_to_hf(hf_dataset);
        }
        _ => println!("Please specify a valid subcommand. Use --help for more information."),
    }
}
