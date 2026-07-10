//! Tunnel supervisor (P2): persisted local port-forwards that auto-start on launch
//! (R5) and reconnect forever with capped exponential backoff (R6).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use parking_lot::Mutex;
use russh::client::Msg;
use russh::Channel;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::{self, JoinHandle};
use tokio::net::{TcpListener, TcpStream};

use crate::ssh::{connect_and_auth, SshConfig, SshHandle};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelRule {
    pub id: String,
    pub name: String,
    pub ssh: SshConfig,
    #[serde(default = "local_host_default")]
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

fn local_host_default() -> String {
    "127.0.0.1".into()
}
fn enabled_default() -> bool {
    true
}

#[derive(Default)]
pub struct TunnelManager {
    running: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl TunnelManager {
    /// Start (or restart) the supervisor for a rule.
    pub fn start(&self, rule: TunnelRule) {
        let id = rule.id.clone();
        if let Some(old) = self.running.lock().remove(&id) {
            old.abort();
        }
        let jh = async_runtime::spawn(supervise(rule));
        self.running.lock().insert(id, jh);
    }

    pub fn stop(&self, id: &str) {
        if let Some(jh) = self.running.lock().remove(id) {
            jh.abort();
        }
    }
}

/// Exponential backoff capped at 30s. Never gives up (infinite reconnect, R6).
pub fn backoff_delay(attempt: u32) -> Duration {
    let secs = (1u64 << attempt.min(5)).min(30); // 1,2,4,8,16,30
    Duration::from_secs(secs)
}

/// Supervise a tunnel forever: (re)connect, serve, on failure back off and retry.
async fn supervise(rule: TunnelRule) {
    let mut attempt: u32 = 0;
    loop {
        match serve_once(&rule).await {
            Ok(()) => attempt = 0, // clean disconnect → reconnect promptly
            Err(_e) => attempt = attempt.saturating_add(1),
        }
        tokio::time::sleep(backoff_delay(attempt)).await;
    }
}

async fn serve_once(rule: &TunnelRule) -> Result<()> {
    let listener = TcpListener::bind((rule.local_host.as_str(), rule.local_port)).await?;
    let handle = Arc::new(connect_and_auth(&rule.ssh).await?);

    let mut health = tokio::time::interval(Duration::from_secs(15));
    health.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (sock, _) = accepted?;
                let h = handle.clone();
                let rh = rule.remote_host.clone();
                let rp = rule.remote_port;
                async_runtime::spawn(async move {
                    let _ = pipe(sock, &h, &rh, rp).await;
                });
            }
            _ = health.tick() => {
                // Liveness probe: if the connection is dead, return to reconnect.
                if handle.channel_open_session().await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

async fn pipe(
    mut sock: TcpStream,
    handle: &SshHandle,
    remote_host: &str,
    remote_port: u16,
) -> Result<()> {
    let channel: Channel<Msg> = handle
        .channel_open_direct_tcpip(remote_host, remote_port as u32, "127.0.0.1", 0)
        .await?;
    let mut stream = channel.into_stream();
    tokio::io::copy_bidirectional(&mut sock, &mut stream).await?;
    Ok(())
}

/// Diagnostic/test helper: open a direct-tcpip channel and read up to `max` bytes.
pub async fn probe_forward(
    ssh: &SshConfig,
    remote_host: &str,
    remote_port: u16,
    max: usize,
) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let handle = connect_and_auth(ssh).await?;
    let channel = handle
        .channel_open_direct_tcpip(remote_host, remote_port as u32, "127.0.0.1", 0)
        .await?;
    let mut stream = channel.into_stream();
    let mut buf = vec![0u8; max];
    let n = stream.read(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

// --- Tauri commands ---

#[tauri::command]
pub fn tunnel_list() -> Vec<TunnelRule> {
    crate::config::load_tunnels()
}

#[tauri::command]
pub fn tunnel_add(
    manager: tauri::State<'_, TunnelManager>,
    rule: TunnelRule,
) -> Result<(), String> {
    let mut rules = crate::config::load_tunnels();
    rules.retain(|r| r.id != rule.id);
    rules.push(rule.clone());
    crate::config::save_tunnels(&rules).map_err(|e| e.to_string())?;
    if rule.enabled {
        manager.start(rule);
    }
    Ok(())
}

#[tauri::command]
pub fn tunnel_remove(
    manager: tauri::State<'_, TunnelManager>,
    id: String,
) -> Result<(), String> {
    manager.stop(&id);
    let mut rules = crate::config::load_tunnels();
    rules.retain(|r| r.id != id);
    crate::config::save_tunnels(&rules).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::backoff_delay;
    use std::time::Duration;

    #[test]
    fn backoff_is_capped_and_monotonic() {
        assert_eq!(backoff_delay(0), Duration::from_secs(1));
        assert_eq!(backoff_delay(1), Duration::from_secs(2));
        assert_eq!(backoff_delay(3), Duration::from_secs(8));
        assert!(backoff_delay(50) <= Duration::from_secs(30));
    }
}
