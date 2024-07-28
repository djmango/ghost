// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod recording;

use log::LevelFilter;
use recording::recording::RecorderState;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_log::{Target, TargetKind};

use crate::recording::{analyze_recording, start_recording, stop_recording};

fn show_window(app: &AppHandle) {
    let windows = app.webview_windows();

    windows
        .values()
        .next()
        .expect("Sorry, no window found")
        .set_focus()
        .expect("Can't Bring Window to Focus");
}

fn main() {
    let recorder_state = Arc::new(RecorderState::new());

    let mut ctx = tauri::generate_context!();
    tauri::Builder::default()
        // .manage(Arc::new(Mutex::new(recording::Recorder::new())))
        .manage(recorder_state)
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = show_window(app);
        }))
        .plugin(tauri_plugin_theme::init(ctx.config_mut()))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(LevelFilter::Debug)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            analyze_recording
        ])
        .run(ctx)
        .expect("error while running tauri application");
}
