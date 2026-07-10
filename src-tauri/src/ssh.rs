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
    /// Local target for remote (-R) forwarded channels, if this is a remote-forward conn.
    forward: Option<(String, u16)>,
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

    // Remote forward (-R): server-initiated channels connect to our local target.
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        if let Some((host, port)) = self.forward.clone() {
            tokio::spawn(async move {
                if let Ok(mut tcp) = tokio::net::TcpStream::connect((host.as_str(), port)).await {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
                }
            });
        }
        Ok(())
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
async fn connect_client(cfg: &SshConfig, forward: Option<(String, u16)>) -> Result<Handle<Client>> {
    // Resolve ~/.ssh/config aliases (HostName / Port / User / IdentityFile).
    let resolved = crate::ssh_config::resolve(&cfg.host);
    let hostname = resolved.hostname.clone().unwrap_or_else(|| cfg.host.clone());
    let port = if cfg.port == 22 { resolved.port.unwrap_or(22) } else { cfg.port };
    let user = if cfg.user.is_empty() || cfg.user == "root" {
        resolved.user.clone().unwrap_or_else(|| cfg.user.clone())
    } else {
        cfg.user.clone()
    };

    let handler = Client { host: hostname.clone(), port, forward };
    let mut handle = client::connect(russh_config(), (hostname.as_str(), port), handler)
        .await
        .with_context(|| format!("connect {hostname}:{port}"))?;

    // 1) ssh-agent — reuse the user's native keys (R2).
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
                    .authenticate_publickey_with(user.as_str(), key, None, &mut agent)
                    .await
                {
                    return Ok(handle);
                }
            }
        }
    }

    // 2) private key files — IdentityFile from ssh config + the usual defaults.
    let mut key_paths = resolved.identity_files.clone();
    if let Some(home) = dirs::home_dir() {
        for name in ["id_ed25519", "id_ecdsa", "id_rsa"] {
            key_paths.push(home.join(".ssh").join(name));
        }
    }
    for kp in key_paths {
        if let Ok(key) = russh::keys::load_secret_key(&kp, None) {
            let kh = russh::keys::PrivateKeyWithHashAlg::new(std::sync::Arc::new(key), None);
            if let Ok(AuthResult::Success) = handle.authenticate_publickey(user.as_str(), kh).await {
                return Ok(handle);
            }
        }
    }

    // 3) password fallback.
    if let Some(pw) = &cfg.password {
        if let Ok(AuthResult::Success) =
            handle.authenticate_password(user.as_str(), pw.as_str()).await
        {
            return Ok(handle);
        }
    }

    Err(anyhow!("authentication failed for {user}@{hostname}"))
}

pub(crate) async fn connect_and_auth(cfg: &SshConfig) -> Result<Handle<Client>> {
    connect_client(cfg, None).await
}

/// Connect with a local target for remote (-R) forwarded channels.
pub(crate) async fn connect_and_auth_fwd(
    cfg: &SshConfig,
    forward: (String, u16),
) -> Result<Handle<Client>> {
    connect_client(cfg, Some(forward)).await
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

/// Open an authenticated connection + an interactive shell channel.
async fn open_shell(
    config: &SshConfig,
    cols: u16,
    rows: u16,
) -> anyhow::Result<(SshHandle, russh::Channel<russh::client::Msg>)> {
    let handle = connect_and_auth(config).await?;
    let channel = handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;
    Ok((handle, channel))
}

#[tauri::command]
pub async fn ssh_connect(
    manager: tauri::State<'_, SshManager>,
    config: SshConfig,
    cols: u16,
    rows: u16,
    on_output: Channel<Vec<u8>>,
) -> Result<u32, String> {
    // First connect must succeed so bad credentials surface immediately.
    let (handle, channel) = open_shell(&config, cols, rows).await.map_err(|e| format!("{e:#}"))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<SshCmd>();
    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);

    // Supervisor: pump the shell; on unexpected drop, reconnect forever (R6).
    async_runtime::spawn(async move {
        let mut handle = handle;
        let mut channel = channel;
        let (mut cols, mut rows) = (cols, rows);
        'outer: loop {
            // pump the current shell
            loop {
                tokio::select! {
                    cmd = rx.recv() => match cmd {
                        Some(SshCmd::Data(d)) => { let _ = channel.data(&d[..]).await; }
                        Some(SshCmd::Resize(c, r)) => {
                            cols = c; rows = r;
                            let _ = channel.window_change(c as u32, r as u32, 0, 0).await;
                        }
                        Some(SshCmd::Close) | None => { let _ = channel.eof().await; break 'outer; }
                    },
                    msg = channel.wait() => match msg {
                        Some(ChannelMsg::Data { data }) => {
                            if on_output.send(data.to_vec()).is_err() { break 'outer; }
                        }
                        Some(ChannelMsg::ExtendedData { data, .. }) => {
                            if on_output.send(data.to_vec()).is_err() { break 'outer; }
                        }
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break, // channel dead → reconnect
                        _ => {}
                    },
                }
            }

            // reconnect with capped backoff + jitter, forever
            drop(handle);
            let _ = on_output.send(b"\r\n\x1b[33m[disconnected - reconnecting...]\x1b[0m\r\n".to_vec());
            let mut attempt = 0u32;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(crate::tunnel::backoff_delay_jittered(attempt)) => {}
                    cmd = rx.recv() => match cmd {
                        Some(SshCmd::Close) | None => break 'outer,
                        Some(SshCmd::Resize(c, r)) => { cols = c; rows = r; }
                        Some(SshCmd::Data(_)) => {} // dropped while disconnected
                    }
                }
                match open_shell(&config, cols, rows).await {
                    Ok((h, ch)) => {
                        handle = h;
                        channel = ch;
                        let _ = on_output.send(b"\x1b[32m[reconnected]\x1b[0m\r\n".to_vec());
                        break;
                    }
                    Err(_) => attempt = attempt.saturating_add(1),
                }
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
