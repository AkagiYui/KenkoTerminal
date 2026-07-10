//! SFTP file operations (P4). Reuses the SSH core; each session keeps its own
//! authenticated connection + an sftp subsystem channel.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use russh_sftp::client::SftpSession;
use serde::Serialize;

use crate::ssh::{connect_and_auth, SshConfig, SshHandle};

struct SftpConn {
    _handle: SshHandle, // keep the SSH connection alive
    sftp: Arc<SftpSession>,
}

#[derive(Default)]
pub struct SftpManager {
    sessions: Mutex<HashMap<u32, SftpConn>>,
    next_id: AtomicU32,
}

#[derive(Serialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Clone the session's Arc out under the lock so we never hold it across `.await`.
fn session(manager: &SftpManager, id: u32) -> Result<Arc<SftpSession>, String> {
    manager
        .sessions
        .lock()
        .get(&id)
        .map(|c| c.sftp.clone())
        .ok_or_else(|| "no such sftp session".to_string())
}

async fn open_sftp(config: &SshConfig) -> anyhow::Result<(SshHandle, SftpSession)> {
    let handle = connect_and_auth(config).await?;
    let channel = handle.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    let sftp = SftpSession::new(channel.into_stream()).await?;
    Ok((handle, sftp))
}

#[tauri::command]
pub async fn sftp_connect(
    manager: tauri::State<'_, SftpManager>,
    config: SshConfig,
) -> Result<u32, String> {
    let (handle, sftp) = open_sftp(&config).await.map_err(|e| format!("{e:#}"))?;
    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);
    manager.sessions.lock().insert(
        id,
        SftpConn {
            _handle: handle,
            sftp: Arc::new(sftp),
        },
    );
    Ok(id)
}

#[tauri::command]
pub async fn sftp_list(
    manager: tauri::State<'_, SftpManager>,
    id: u32,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    let sftp = session(&manager, id)?;
    let dir = sftp.read_dir(path).await.map_err(|e| e.to_string())?;
    let mut out: Vec<FileEntry> = dir
        .map(|e| {
            let md = e.metadata();
            FileEntry {
                name: e.file_name(),
                is_dir: md.is_dir(),
                size: md.len(),
            }
        })
        .collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

#[tauri::command]
pub async fn sftp_realpath(
    manager: tauri::State<'_, SftpManager>,
    id: u32,
    path: String,
) -> Result<String, String> {
    let sftp = session(&manager, id)?;
    sftp.canonicalize(path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sftp_read(
    manager: tauri::State<'_, SftpManager>,
    id: u32,
    path: String,
) -> Result<Vec<u8>, String> {
    let sftp = session(&manager, id)?;
    sftp.read(path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sftp_write(
    manager: tauri::State<'_, SftpManager>,
    id: u32,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let sftp = session(&manager, id)?;
    sftp.write(path, &data).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sftp_mkdir(
    manager: tauri::State<'_, SftpManager>,
    id: u32,
    path: String,
) -> Result<(), String> {
    let sftp = session(&manager, id)?;
    sftp.create_dir(path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sftp_remove(
    manager: tauri::State<'_, SftpManager>,
    id: u32,
    path: String,
    is_dir: bool,
) -> Result<(), String> {
    let sftp = session(&manager, id)?;
    if is_dir {
        sftp.remove_dir(path).await.map_err(|e| e.to_string())
    } else {
        sftp.remove_file(path).await.map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn sftp_rename(
    manager: tauri::State<'_, SftpManager>,
    id: u32,
    from: String,
    to: String,
) -> Result<(), String> {
    let sftp = session(&manager, id)?;
    sftp.rename(from, to).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sftp_close(manager: tauri::State<'_, SftpManager>, id: u32) -> Result<(), String> {
    manager.sessions.lock().remove(&id);
    Ok(())
}

/// Test helper: connect + list a directory (used by the smoke test).
pub async fn list_once(config: &SshConfig, path: &str) -> anyhow::Result<Vec<String>> {
    let (_handle, sftp) = open_sftp(config).await?;
    let dir = sftp.read_dir(path.to_string()).await?;
    Ok(dir.map(|e| e.file_name()).collect())
}
