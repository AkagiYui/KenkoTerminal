//! Persisted app config (P2): tunnel rules live under the OS config dir.

use std::path::PathBuf;

use anyhow::Result;

use crate::tunnel::TunnelRule;

fn config_dir() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    p.push("KenkoTerminal");
    p
}

fn tunnels_path() -> PathBuf {
    let mut p = config_dir();
    p.push("tunnels.json");
    p
}

pub fn load_tunnels() -> Vec<TunnelRule> {
    std::fs::read(tunnels_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_tunnels(rules: &[TunnelRule]) -> Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(tunnels_path(), serde_json::to_vec_pretty(rules)?)?;
    Ok(())
}

// --- generic JSON config helpers (connections, groups, settings, ...) ---

pub fn config_path(name: &str) -> PathBuf {
    let mut p = config_dir();
    p.push(name);
    p
}

pub fn load_json<T: serde::de::DeserializeOwned + Default>(name: &str) -> T {
    std::fs::read(config_path(name))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_json<T: serde::Serialize>(name: &str, value: &T) -> Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(config_path(name), serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
