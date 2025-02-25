# Ghost - Computer-Use AI Training Data Recorder

## Overview

Ghost is a cross-platform screen and input recording application designed to capture high-quality training data for AI models. Built with Tauri, Rust, and React, Ghost provides a seamless recording experience across Windows, macOS, and Linux.

## Features

- **Screen Recording**: Capture high-quality screen recordings using FFmpeg
- **Input Tracking**: Record mouse movements, clicks, and keyboard inputs
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Secure Authentication**: JWT-based authentication system
- **Modern UI**: Clean, responsive interface built with React and Tailwind CSS
- **Efficient Storage**: Optimized data storage format for AI training purposes

## Architecture

Ghost is built using the following technologies:

### Backend (Rust)
- **Tauri**: Cross-platform framework for building desktop applications
- **FFmpeg**: Used for screen recording via the ffmpeg-sidecar crate
- **rdev**: Captures input events (mouse movements, clicks, keyboard inputs)
- **Serde**: Serialization/deserialization of data

### Frontend (TypeScript/React)
- **React**: UI framework
- **Tailwind CSS**: Utility-first CSS framework
- **shadcn/ui**: Component library
- **Vite**: Build tool and development server

## Project Structure

```
ghost/
├── src/                  # Frontend React code
├── src-tauri/            # Rust backend code
│   ├── src/
│   │   ├── main.rs       # Entry point for the Tauri application
│   │   ├── auth.rs       # Authentication functionality
│   │   ├── recording/    # Screen and input recording functionality
│   │   └── types/        # Type definitions
│   ├── Cargo.toml        # Rust dependencies
│   └── tauri.conf.json5  # Tauri configuration
├── public/               # Static assets
└── package.json          # Frontend dependencies
```

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js](https://nodejs.org/) (or [Bun](https://bun.sh/))
- [FFmpeg](https://ffmpeg.org/download.html)

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/invisibility_inc/ghost.git
   cd ghost
   ```

2. Install frontend dependencies:
   ```bash
   bun install
   # or
   npm install
   ```

3. Run the development server:
   ```bash
   bun run tauri dev
   # or
   npm run tauri dev
   ```

### Building for Production

```bash
bun run tauri build
# or
npm run tauri build
```

## How It Works

Ghost uses FFmpeg for screen recording and the rdev library to capture input events. These events are synchronized and stored in a format optimized for AI training. The application provides a user-friendly interface for starting and stopping recordings, as well as managing recorded sessions.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## About Invisibility Inc

Ghost is developed by [Invisibility Inc](https://i.inc), a company focused on developing AI-powered tools and solutions.
