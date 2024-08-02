// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod recording;
mod types;

use log::{debug, LevelFilter};
use recording::recording::RecorderState;
use std::{fs, sync::Arc};
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

use crate::recording::{start_recording, stop_recording};

fn main() {
    tauri::Builder::default()
        // .manage(RecorderState::new())
        .setup(|app| {
            app.manage(RecorderState::new(app.handle()));

            fs::create_dir_all(app.path().app_data_dir().unwrap()).unwrap();

            let base_dir = app.path().app_data_dir().unwrap();

            debug!("Custom directory: {:?}", base_dir);

            Ok(())
        })
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
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
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
