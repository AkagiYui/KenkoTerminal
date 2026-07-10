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
