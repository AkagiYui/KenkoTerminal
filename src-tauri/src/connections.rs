//! Saved connections + credential vault.
//!
//! Connection metadata lives in `connections.json`; passwords are stored in the
//! OS keychain (macOS Keychain / Windows Credential Manager) via `keyring`, keyed
//! by connection id — no plaintext secrets on disk.

use serde::{Deserialize, Serialize};

const SERVICE: &str = "com.akagiyui.kenkoterminal";
const FILE: &str = "connections.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub kind: String, // "ssh" | "serial" | "telnet" | "local"
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub path: Option<String>, // serial device
    #[serde(default)]
    pub baud: Option<u32>,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub group: Option<String>,
}

fn entry(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conn_list() -> Vec<Connection> {
    crate::config::load_json(FILE)
}

/// Save (or update) a connection. `password: Some("")` clears it; `None` leaves it.
#[tauri::command]
pub fn conn_save(mut conn: Connection, password: Option<String>) -> Result<(), String> {
    if let Some(pw) = password {
        let e = entry(&conn.id)?;
        if pw.is_empty() {
            let _ = e.delete_credential();
            conn.has_password = false;
        } else {
            e.set_password(&pw).map_err(|err| err.to_string())?;
            conn.has_password = true;
        }
    }
    let mut list: Vec<Connection> = crate::config::load_json(FILE);
    list.retain(|c| c.id != conn.id);
    list.push(conn);
    crate::config::save_json(FILE, &list).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn conn_delete(id: String) -> Result<(), String> {
    if let Ok(e) = entry(&id) {
        let _ = e.delete_credential();
    }
    let mut list: Vec<Connection> = crate::config::load_json(FILE);
    list.retain(|c| c.id != id);
    crate::config::save_json(FILE, &list).map_err(|e| e.to_string())
}

/// Fetch a saved password from the OS keychain (used when launching a saved conn).
#[tauri::command]
pub fn conn_password(id: String) -> Option<String> {
    entry(&id).ok().and_then(|e| e.get_password().ok())
}
