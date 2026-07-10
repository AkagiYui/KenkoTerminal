//! SSH transport (P1).
//!
//! russh 0.54: authentication reuses the native **ssh-agent** (R2 "复用本机密钥"),
//! with a password fallback; host keys are verified against `~/.ssh/known_hosts`
//! with trust-on-first-use. Provides both an interactive shell (for the terminal)
//! and a one-shot `exec_capture` (used by tests now, and by monitoring/batch later).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use russh::client::{self, AuthResult, Handle};
use russh::keys::agent::client::AgentClient;
use russh::keys::known_hosts::{check_known_hosts, learn_known_hosts};
use russh::keys::ssh_key;
use russh::ChannelMsg;
use serde::{Deserialize, Serialize};
use tauri::async_runtime;
use tauri::ipc::Channel;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub password: Option<String>,
}

fn default_port() -> u16 {
    22
}

/// russh client handler — verifies the server host key (TOFU over known_hosts).
pub(crate) struct Client {
    host: String,
    port: u16,
}

/// Authenticated client connection handle (used by the tunnel supervisor).
pub(crate) type SshHandle = Handle<Client>;

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &ssh_key::PublicKey) -> Result<bool, Self::Error> {
        match check_known_hosts(&self.host, self.port, key) {
            Ok(true) => Ok(true), // known & matches
            Ok(false) => {
                // Unknown host — trust on first use, then remember it.
                let _ = learn_known_hosts(&self.host, self.port, key);
                Ok(true)
            }
            Err(_) => Ok(false), // present but changed → reject
        }
    }
}

fn russh_config() -> Arc<client::Config> {
    Arc::new(client::Config {
        // Keepalives let us detect a dead link quickly (feeds P2 reconnect).
        keepalive_interval: Some(Duration::from_secs(20)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    })
}

/// Connect and authenticate. Auth order: ssh-agent → password.
pub(crate) async fn connect_and_auth(cfg: &SshConfig) -> Result<Handle<Client>> {
    let handler = Client {
        host: cfg.host.clone(),
        port: cfg.port,
    };
    let mut handle = client::connect(russh_config(), (cfg.host.as_str(), cfg.port), handler)
        .await
        .with_context(|| format!("connect {}:{}", cfg.host, cfg.port))?;

    // 1) ssh-agent — reuse the user's native keys (R2). Cross-platform:
    //    SSH_AUTH_SOCK on unix, the OpenSSH named pipe on Windows.
    #[cfg(unix)]
    let agent = AgentClient::connect_env().await.ok();
    #[cfg(windows)]
    let agent = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
        .await
        .ok();

    if let Some(mut agent) = agent {
        if let Ok(identities) = agent.request_identities().await {
            for key in identities {
                if let Ok(AuthResult::Success) = handle
                    .authenticate_publickey_with(cfg.user.as_str(), key, None, &mut agent)
                    .await
                {
                    return Ok(handle);
                }
            }
        }
    }

    // 2) password fallback.
    if let Some(pw) = &cfg.password {
        if let Ok(AuthResult::Success) =
            handle.authenticate_password(cfg.user.as_str(), pw.as_str()).await
        {
            return Ok(handle);
        }
    }

    Err(anyhow!("authentication failed for {}@{}", cfg.user, cfg.host))
}

/// Run a single command and capture (stdout+stderr, exit_code). Used by tests and
/// later by remote probing / batch exec.
pub async fn exec_capture(cfg: &SshConfig, command: &str) -> Result<(String, u32)> {
    let handle = connect_and_auth(cfg).await?;
    let mut channel = handle.channel_open_session().await?;
    channel.exec(true, command).await?;

    let mut out = Vec::new();
    let mut code = 0u32;
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { data }) => out.extend_from_slice(&data),
            Some(ChannelMsg::ExtendedData { data, .. }) => out.extend_from_slice(&data),
            Some(ChannelMsg::ExitStatus { exit_status }) => code = exit_status,
            Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }
    Ok((String::from_utf8_lossy(&out).into_owned(), code))
}

// ---------------------------------------------------------------------------
// Tauri command layer: interactive shell sessions
// ---------------------------------------------------------------------------

enum SshCmd {
    Data(Vec<u8>),
    Resize(u16, u16),
    Close,
}

struct SshSessionHandle {
    tx: mpsc::UnboundedSender<SshCmd>,
}

#[derive(Default)]
pub struct SshManager {
    sessions: Mutex<HashMap<u32, SshSessionHandle>>,
    next_id: AtomicU32,
}

#[tauri::command]
pub async fn ssh_connect(
    manager: tauri::State<'_, SshManager>,
    config: SshConfig,
    cols: u16,
    rows: u16,
    on_output: Channel<Vec<u8>>,
) -> Result<u32, String> {
    let handle = connect_and_auth(&config).await.map_err(|e| format!("{e:#}"))?;
    let mut channel = handle.channel_open_session().await.map_err(|e| e.to_string())?;
    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
        .map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

    let (tx, mut rx) = mpsc::unbounded_channel::<SshCmd>();
    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);

    async_runtime::spawn(async move {
        // Keep the connection handle alive for the lifetime of the session.
        let _handle = handle;
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(SshCmd::Data(d)) => { let _ = channel.data(&d[..]).await; }
                    Some(SshCmd::Resize(c, r)) => {
                        let _ = channel.window_change(c as u32, r as u32, 0, 0).await;
                    }
                    Some(SshCmd::Close) | None => {
                        let _ = channel.eof().await;
                        break;
                    }
                },
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { data }) => {
                        if on_output.send(data.to_vec()).is_err() { break; }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if on_output.send(data.to_vec()).is_err() { break; }
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    _ => {}
                },
            }
        }
    });

    manager.sessions.lock().insert(id, SshSessionHandle { tx });
    Ok(id)
}

#[tauri::command]
pub fn ssh_write(manager: tauri::State<'_, SshManager>, id: u32, data: String) -> Result<(), String> {
    let sessions = manager.sessions.lock();
    let s = sessions.get(&id).ok_or("no such ssh session")?;
    s.tx
        .send(SshCmd::Data(data.into_bytes()))
        .map_err(|_| "ssh session closed".to_string())
}

#[tauri::command]
pub fn ssh_resize(
    manager: tauri::State<'_, SshManager>,
    id: u32,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let sessions = manager.sessions.lock();
    let s = sessions.get(&id).ok_or("no such ssh session")?;
    let _ = s.tx.send(SshCmd::Resize(cols, rows));
    Ok(())
}

#[tauri::command]
pub fn ssh_close(manager: tauri::State<'_, SshManager>, id: u32) -> Result<(), String> {
    if let Some(s) = manager.sessions.lock().remove(&id) {
        let _ = s.tx.send(SshCmd::Close);
    }
    Ok(())
}
