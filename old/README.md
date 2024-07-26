# i.inc Desktop Event (devent) Recorder

Version 1.0

Developed by Invisibility Inc - Sulaiman Ghori

## Description

The i.inc Desktop Event (devent) Recorder is a tool designed to track cursor movements and capture screenshots. It provides functionality for recording desktop events, displaying random screenshots from recorded data, and uploading datasets to Hugging Face.

## Installation

TODO

## Usage

The devent Recorder offers three main commands: `record`, `display`, and `upload`.

### Record

Records cursor movement and captures screenshots for a specified duration.

```

devent record [OPTIONS]

```

Options:

- `-d, --duration <SECONDS>`: Recording duration in seconds (default: 10)
- `-o, --output <DIR>`: Output directory for recordings (default: "output")

Example:

```

devent record -d 30 -o my_recordings

```

### Display

Displays a random screenshot from a recorded dataset.

```

devent display [OPTIONS]

```

Options:

- `--dataset <DATASET>`: Specific dataset to use (optional)

Example:

```

devent display --dataset my_dataset

```

### Upload

Uploads a recorded dataset to Hugging Face.

```

devent upload --dataset <DATASET> --hf-dataset <HF_DATASET>

```

Options:

- `--dataset <DATASET>`: Local dataset to upload (required)
- `--hf-dataset <HF_DATASET>`: Hugging Face dataset name (required)

Example:

```

devent upload --dataset my_local_dataset --hf-dataset my_hf_dataset

```

## Features

- Screen recording with cursor movement tracking
- Random screenshot display from recorded datasets
- Dataset upload to Hugging Face

## Requirements

- ffmpeg
