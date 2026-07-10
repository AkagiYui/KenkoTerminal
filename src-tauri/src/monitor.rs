//! Remote system probe + resource monitor (P5).
//!
//! Reuses the SSH core. Monitoring streams `/proc` over a single long-lived exec
//! channel (a shell loop) — no agent to deploy. CPU% is a delta between samples.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::Mutex;
use russh::ChannelMsg;
use serde::Serialize;
use tauri::async_runtime::{self, JoinHandle};
use tauri::ipc::Channel;

use crate::ssh::{connect_and_auth, exec_capture, SshConfig};

#[derive(Serialize)]
pub struct SystemInfo {
    pub uname: String,
    pub os_release: String,
}

/// One-shot probe of the remote OS (R7).
#[tauri::command]
pub async fn probe_system(config: SshConfig) -> Result<SystemInfo, String> {
    let (uname, _) = exec_capture(&config, "uname -a")
        .await
        .map_err(|e| format!("{e:#}"))?;
    let (os_release, _) = exec_capture(
        &config,
        "(. /etc/os-release 2>/dev/null && echo \"$PRETTY_NAME\"); uptime",
    )
    .await
    .map_err(|e| format!("{e:#}"))?;
    Ok(SystemInfo {
        uname: uname.trim().to_string(),
        os_release: os_release.trim().to_string(),
    })
}

#[derive(Serialize, Clone, Default)]
pub struct Sample {
    pub cpu: f32,
    pub mem_used_kb: u64,
    pub mem_total_kb: u64,
}

#[derive(Default)]
pub struct MonitorManager {
    running: Mutex<HashMap<u32, JoinHandle<()>>>,
    next_id: AtomicU32,
}

// Emits, every 2s: "<cpu_total> <cpu_idle>" then "<mem_total_kb> <mem_avail_kb>" then "__T__".
const MON_CMD: &str = "while :; do \
awk '/^cpu /{print $2+$3+$4+$5+$6+$7+$8, $5}' /proc/stat; \
awk '/^MemTotal:/{t=$2} /^MemAvailable:/{a=$2} END{print t, a}' /proc/meminfo; \
echo __T__; sleep 2; done";

fn parse_sample(cpu_line: &str, mem_line: &str, prev: &mut Option<(u64, u64)>) -> Option<Sample> {
    let cpu: Vec<u64> = cpu_line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let mem: Vec<u64> = mem_line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if cpu.len() != 2 || mem.len() != 2 {
        return None;
    }
    let (total, idle) = (cpu[0], cpu[1]);
    let cpu_pct = match *prev {
        Some((pt, pi)) => {
            let dt = total.saturating_sub(pt);
            let di = idle.saturating_sub(pi);
            if dt > 0 {
                ((1.0 - di as f32 / dt as f32) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        }
        None => 0.0,
    };
    *prev = Some((total, idle));
    Some(Sample {
        cpu: cpu_pct,
        mem_used_kb: mem[0].saturating_sub(mem[1]),
        mem_total_kb: mem[0],
    })
}

#[tauri::command]
pub async fn monitor_start(
    manager: tauri::State<'_, MonitorManager>,
    config: SshConfig,
    on_sample: Channel<Sample>,
) -> Result<u32, String> {
    let handle = connect_and_auth(&config).await.map_err(|e| format!("{e:#}"))?;
    let mut channel = handle.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.exec(true, MON_CMD).await.map_err(|e| e.to_string())?;

    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);
    let jh = async_runtime::spawn(async move {
        let _handle = handle;
        let mut buf = String::new();
        let mut lines: Vec<String> = Vec::new();
        let mut prev: Option<(u64, u64)> = None;
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) => {
                    buf.push_str(&String::from_utf8_lossy(&data));
                    while let Some(nl) = buf.find('\n') {
                        let line: String = buf.drain(..=nl).collect();
                        let line = line.trim().to_string();
                        if line == "__T__" {
                            if lines.len() >= 2 {
                                if let Some(s) = parse_sample(&lines[0], &lines[1], &mut prev) {
                                    if on_sample.send(s).is_err() {
                                        return;
                                    }
                                }
                            }
                            lines.clear();
                        } else if !line.is_empty() {
                            lines.push(line);
                        }
                    }
                }
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                _ => {}
            }
        }
    });
    manager.running.lock().insert(id, jh);
    Ok(id)
}

#[tauri::command]
pub fn monitor_stop(manager: tauri::State<'_, MonitorManager>, id: u32) -> Result<(), String> {
    if let Some(jh) = manager.running.lock().remove(&id) {
        jh.abort();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_sample;

    #[test]
    fn cpu_delta_and_mem() {
        let mut prev = None;
        // first sample seeds prev, cpu = 0
        let s0 = parse_sample("1000 900", "2000 500", &mut prev).unwrap();
        assert_eq!(s0.cpu, 0.0);
        assert_eq!(s0.mem_total_kb, 2000);
        assert_eq!(s0.mem_used_kb, 1500);
        // next: dtotal=100, didle=50 -> 50% busy
        let s1 = parse_sample("1100 950", "2000 500", &mut prev).unwrap();
        assert!((s1.cpu - 50.0).abs() < 0.01, "cpu={}", s1.cpu);
    }
}
