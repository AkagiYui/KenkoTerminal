//! Batch / fleet exec (P6, R13): run one command across a host group in parallel
//! via one-shot `exec` (not the interactive PTY), with a concurrency cap (forks)
//! and a per-host recap. Results stream back as each host finishes.

use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::async_runtime;
use tauri::ipc::Channel;
use tokio::sync::Semaphore;

use crate::ssh::{exec_capture, SshConfig};

#[derive(Debug, Clone, Deserialize)]
pub struct BatchTarget {
    pub label: String,
    pub ssh: SshConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchResult {
    pub label: String,
    pub host: String,
    pub ok: bool,
    pub exit_code: i64,
    pub output: String,
    pub ms: u64,
}

async fn run_one(target: &BatchTarget, command: &str) -> BatchResult {
    let start = Instant::now();
    let (ok, exit_code, output) = match exec_capture(&target.ssh, command).await {
        Ok((out, code)) => (code == 0, code as i64, out),
        Err(e) => (false, -1, format!("{e:#}")), // unreachable / auth / exec error
    };
    BatchResult {
        label: target.label.clone(),
        host: target.ssh.host.clone(),
        ok,
        exit_code,
        output,
        ms: start.elapsed().as_millis() as u64,
    }
}

/// Core fan-out used by the command (streaming) and tests (collecting).
pub async fn run_batch(targets: Vec<BatchTarget>, command: String, forks: usize) -> Vec<BatchResult> {
    let sem = Arc::new(Semaphore::new(forks.max(1)));
    let command = Arc::new(command);
    let mut handles = Vec::with_capacity(targets.len());
    for target in targets {
        let sem = sem.clone();
        let command = command.clone();
        handles.push(async_runtime::spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore");
            run_one(&target, &command).await
        }));
    }
    let mut out = Vec::new();
    for h in handles {
        if let Ok(r) = h.await {
            out.push(r);
        }
    }
    out
}

#[tauri::command]
pub async fn batch_run(
    targets: Vec<BatchTarget>,
    command: String,
    forks: Option<usize>,
    on_result: Channel<BatchResult>,
) -> Result<(), String> {
    let sem = Arc::new(Semaphore::new(forks.unwrap_or(8).max(1)));
    let command = Arc::new(command);
    let mut handles = Vec::with_capacity(targets.len());
    for target in targets {
        let sem = sem.clone();
        let command = command.clone();
        let on_result = on_result.clone();
        handles.push(async_runtime::spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore");
            let r = run_one(&target, &command).await;
            let _ = on_result.send(r); // stream each result as it lands
        }));
    }
    for h in handles {
        let _ = h.await;
    }
    Ok(())
}
