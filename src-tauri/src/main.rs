// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod recording;

use log::LevelFilter;
use tauri::{AppHandle, Manager, Window};
use tauri_plugin_log::{Target, TargetKind};

use crate::recording::start_recording;

#[tauri::command]
fn get_monitors(window: Window) -> String {
    window
        .current_monitor()
        .unwrap()
        .unwrap()
        .name()
        .unwrap()
        .to_string()
}

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
    // tracing_subscriber::fmt::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = show_window(app);
        }))
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
        .invoke_handler(tauri::generate_handler![get_monitors, start_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
