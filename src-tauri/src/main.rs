// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod recording;

use tauri::{AppHandle, Manager, Window};

use crate::recording::start_recording;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

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
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = show_window(app);
        }))
        .invoke_handler(tauri::generate_handler![
            greet,
            get_monitors,
            start_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
