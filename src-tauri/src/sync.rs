//! Config sync over WebDAV (or any HTTP PUT/GET endpoint).
//!
//! Bundles the non-secret config (connections metadata + tunnel rules) into one JSON
//! and PUT/GETs it. Passwords stay in the OS keychain and are NOT synced.

use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct Bundle {
    connections: serde_json::Value,
    tunnels: serde_json::Value,
}

fn basic_auth(user: &str, pass: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
    format!("Basic {token}")
}

#[tauri::command]
pub fn config_sync_push(url: String, user: String, password: String) -> Result<(), String> {
    let bundle = Bundle {
        connections: crate::config::load_json("connections.json"),
        tunnels: crate::config::load_json("tunnels.json"),
    };
    let body = serde_json::to_vec(&bundle).map_err(|e| e.to_string())?;
    let mut req = ureq::put(&url);
    if !user.is_empty() {
        req = req.set("Authorization", &basic_auth(&user, &password));
    }
    req.send_bytes(&body).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn config_sync_pull(url: String, user: String, password: String) -> Result<(), String> {
    let mut req = ureq::get(&url);
    if !user.is_empty() {
        req = req.set("Authorization", &basic_auth(&user, &password));
    }
    let resp = req.call().map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    use std::io::Read;
    resp.into_reader()
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    let bundle: Bundle = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    crate::config::save_json("connections.json", &bundle.connections).map_err(|e| e.to_string())?;
    crate::config::save_json("tunnels.json", &bundle.tunnels).map_err(|e| e.to_string())?;
    Ok(())
}
