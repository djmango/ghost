import os
import ffmpeg
import csv
from datetime import datetime

def get_video_dimensions(video_path):
    probe = ffmpeg.probe(video_path)
    video_info = next(s for s in probe['streams'] if s['codec_type'] == 'video')
    return int(video_info['width']), int(video_info['height'])

def read_events(csv_path):
    events = []
    with open(csv_path, 'r') as csvfile:
        reader = csv.DictReader(csvfile)
        for row in reader:
            if row['event_type'] == 'ButtonPress(Left)':
                events.append({
                    'timestamp': int(row['timestamp']),
                    'x': float(row['mouse_x']),
                    'y': float(row['mouse_y'])
                })
    return events

def process_folder(folder_path):
    # Get the path to the recordings.mkv file
    video_path = os.path.join(folder_path, "recording.mkv")
    
    # Get the path to the events.csv file
    csv_path = os.path.join(folder_path, "events.csv")
    
    # Read events from CSV
    events = read_events(csv_path)
    
    # Prepare ffmpeg inputs
    input_video = ffmpeg.input(video_path)
    
    # Get video start time from timestamps.txt file
    timestamps_path = os.path.join(folder_path, "timestamps.txt")
    with open(timestamps_path, 'r') as f:
        # Skip the first line (header)
        next(f)
        # Read the first timestamp
        video_start_time = int(f.readline().strip())
    
    # Apply drawbox filters
    overlays = input_video
    for event in events:
        x, y = event['x'], event['y']  # Adjust y-coordinate
        w, h = 30, 30  # Size of the box around the click
        color = 'red'
        thickness = 2
        st = (event['timestamp'] - video_start_time) / 1000
        et = st + 0.1
        overlays = (
            overlays
            .drawbox(
                x-w/2, y-h/2, w, h,
                color=color, thickness=thickness,
                enable=f'gte(t,{st})*lte(t,{et})'
            )
        )
    
    # Set output filename
    output_filename = os.path.join(folder_path, "output_with_boxes.mp4")
    
    # Create output
    output = (
        overlays
        .output(output_filename)
        .overwrite_output()
    )
    
    # Run ffmpeg command
    output.run()

def main():
    # Get the current directory
    output = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'output')
    
    # Get all subdirectories in the current directory
    subdirs = [d for d in os.listdir(output) if os.path.isdir(os.path.join(output, d))]
    
    # Process each subdirectory
    for subdir in subdirs:
        folder_path = os.path.join(output, subdir)
        print(f"Processing folder: {folder_path}")
        process_folder(folder_path)

if __name__ == "__main__":
    main()