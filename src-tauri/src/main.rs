// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod recording;

use crate::auth::{parse_jwt_from_url, save_jwt_to_store};
use crate::recording::{start_recording, stop_recording};
use log::LevelFilter;
use recording::recording::RecorderState;
use tauri_plugin_log::{Target, TargetKind};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_store::StoreCollection;
use std::path::PathBuf;
use serde_json::json;
use std::fs;

struct AppState {
    username: Mutex<String>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            username: Mutex::new(String::new()),
        }
    }
}

#[tauri::command]
fn set_email(app_handle: tauri::AppHandle, email: String) -> Result<(), String> {
    let stores = app_handle.state::<StoreCollection<tauri::Wry>>();
    let path: PathBuf = app_handle.path().app_data_dir()
        .map(|p| p.join("store.bin"))
        .unwrap_or_else(|e| PathBuf::from("store.bin"));

    // Ensure the directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tauri_plugin_store::with_store(app_handle.clone(), stores, path, |store| {
        store.insert("email".to_string(), json!(email))?;
        store.save()?;
        Ok(())
    })
    .map_err(|e| format!("Store operation failed: {}", e))
}

fn main() {
    let mut ctx = tauri::generate_context!();
    tauri::Builder::default()
        .manage(RecorderState::new())
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
        .invoke_handler(tauri::generate_handler![start_recording, stop_recording, set_email])
        .run(ctx)
        .expect("error while running tauri application");
}

