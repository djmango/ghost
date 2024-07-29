use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_store::{with_store, StoreCollection};
use tauri::Wry;
use url::Url;
use serde_json::json;
use std::path::PathBuf;

pub fn get_jwt_from_store(app: &AppHandle) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let stores = app.state::<StoreCollection<Wry>>();
    let path: PathBuf = PathBuf::from("store.bin");

    with_store(app.clone(), stores, path, |store| {
        let jwt = store.get("jwt_token")
            .and_then(|v| v.as_str().map(String::from));
        Ok(jwt)
    }).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
pub fn parse_jwt_from_url(url: &str) -> Option<String> {
    if let Ok(parsed_url) = Url::parse(url) {
        if parsed_url.scheme() == "invisibility" && parsed_url.path() == "/auth_callback" {
            return parsed_url.query_pairs()
                .find(|(key, _)| key == "token")
                .map(|(_, value)| value.into_owned());
        }
    }
    None
}

pub fn save_jwt_to_store(app: &AppHandle, jwt: &str) -> Result<(), Box<dyn std::error::Error>> {
    let stores = app.state::<StoreCollection<Wry>>();
    let path: PathBuf = PathBuf::from("store.bin");

    with_store(app.clone(), stores, path, |store| {
        store.insert("jwt_token".to_string(), json!(jwt))?;
        store.save()?;
        Ok(())
    }).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}