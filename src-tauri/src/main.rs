// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod recording;

use crate::auth::{parse_jwt_from_url, save_jwt_to_store};
use crate::recording::{start_recording, stop_recording};
use log::LevelFilter;
use recording::recording::RecorderState;
use tauri::Listener;
use tauri_plugin_log::{Target, TargetKind};

fn main() {
    let mut ctx = tauri::generate_context!();
    tauri::Builder::default()
        .manage(RecorderState::new())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            app.listen("tauri://deep-link", move |event| {
                let payload = event.payload();
                if let Some(jwt) = parse_jwt_from_url(payload) {
                    match save_jwt_to_store(&app_handle, &jwt) {
                        Ok(_) => println!("success"),
                        Err(_) => println!("Fail"),
                    }
                }
            });
            Ok(())
        })
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        // .plugin(tauri_plugin_window_state::Builder::default().build())
        // .plugin(tauri_plugin_theme::init(ctx.config_mut()))
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
        .run(ctx)
        .expect("error while running tauri application");
}

