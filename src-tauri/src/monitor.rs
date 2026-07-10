//! Remote system probe + resource monitor (P5).
//!
//! Streams CPU / memory / disk / network + top processes over ONE long-lived exec
//! channel (a shell loop over /proc + df + ps) — no agent to deploy. CPU% and net
//! rate are deltas between samples.

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
pub struct Proc {
    pub cpu: f32,
    pub name: String,
}

#[derive(Serialize, Clone, Default)]
pub struct Sample {
    pub cpu: f32,
    pub mem_used_kb: u64,
    pub mem_total_kb: u64,
    pub disk_used_kb: u64,
    pub disk_total_kb: u64,
    pub net_rx_bps: u64,
    pub net_tx_bps: u64,
    pub procs: Vec<Proc>,
}

#[derive(Default)]
pub struct MonitorManager {
    running: Mutex<HashMap<u32, JoinHandle<()>>>,
    next_id: AtomicU32,
}

const INTERVAL_SECS: u64 = 2;

// Per tick: cpu(total idle) / mem(total avail) / net(rx tx) / disk(total used) /
// __P__ / up to 5 "pcpu comm" process lines / __T__.
const MON_CMD: &str = "while :; do \
awk '/^cpu /{print $2+$3+$4+$5+$6+$7+$8, $5}' /proc/stat; \
awk '/^MemTotal:/{t=$2}/^MemAvailable:/{a=$2}END{print t, a}' /proc/meminfo; \
awk 'NR>2{rx+=$2; tx+=$10}END{print rx+0, tx+0}' /proc/net/dev; \
df -kP / 2>/dev/null | awk 'NR==2{print $2, $3}'; \
echo __P__; \
ps -eo pcpu=,comm= --sort=-pcpu 2>/dev/null | head -n 5; \
echo __T__; sleep 2; done";

#[derive(Default)]
struct Prev {
    cpu: Option<(u64, u64)>,
    net: Option<(u64, u64)>,
}

fn nums(s: &str) -> Vec<u64> {
    s.split_whitespace().filter_map(|x| x.parse().ok()).collect()
}

fn parse_tick(lines: &[String], prev: &mut Prev) -> Option<Sample> {
    let p_idx = lines.iter().position(|l| l == "__P__")?;
    let fixed = &lines[..p_idx];
    let proc_lines = &lines[p_idx + 1..];
    if fixed.len() < 4 {
        return None;
    }
    let cpu = nums(&fixed[0]);
    let mem = nums(&fixed[1]);
    let net = nums(&fixed[2]);
    let disk = nums(&fixed[3]);
    if cpu.len() < 2 || mem.len() < 2 {
        return None;
    }
    let (total, idle) = (cpu[0], cpu[1]);
    let cpu_pct = match prev.cpu {
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
    prev.cpu = Some((total, idle));

    let (net_rx_bps, net_tx_bps) = if net.len() >= 2 {
        let (rx, tx) = (net[0], net[1]);
        let rate = match prev.net {
            Some((prx, ptx)) => (
                rx.saturating_sub(prx) / INTERVAL_SECS,
                tx.saturating_sub(ptx) / INTERVAL_SECS,
            ),
            None => (0, 0),
        };
        prev.net = Some((rx, tx));
        rate
    } else {
        (0, 0)
    };

    let (disk_total_kb, disk_used_kb) = if disk.len() >= 2 {
        (disk[0], disk[1])
    } else {
        (0, 0)
    };

    let procs: Vec<Proc> = proc_lines
        .iter()
        .filter_map(|l| {
            let mut it = l.trim().splitn(2, char::is_whitespace);
            let cpu = it.next()?.parse::<f32>().ok()?;
            let name = it.next().unwrap_or("").trim().to_string();
            (!name.is_empty()).then_some(Proc { cpu, name })
        })
        .collect();

    Some(Sample {
        cpu: cpu_pct,
        mem_used_kb: mem[0].saturating_sub(mem[1]),
        mem_total_kb: mem[0],
        disk_used_kb,
        disk_total_kb,
        net_rx_bps,
        net_tx_bps,
        procs,
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
        let mut prev = Prev::default();
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) => {
                    buf.push_str(&String::from_utf8_lossy(&data));
                    while let Some(nl) = buf.find('\n') {
                        let line: String = buf.drain(..=nl).collect();
                        let line = line.trim().to_string();
                        if line == "__T__" {
                            if let Some(s) = parse_tick(&lines, &mut prev) {
                                if on_sample.send(s).is_err() {
                                    return;
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
    use super::*;

    #[test]
    fn parse_tick_full() {
        let mut prev = Prev::default();
        let t1: Vec<String> = ["1000 900", "2000 500", "1000 2000", "10000 4000", "__P__", "50.0 firefox"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let s0 = parse_tick(&t1, &mut prev).unwrap();
        assert_eq!(s0.cpu, 0.0); // first sample seeds
        assert_eq!(s0.mem_used_kb, 1500);
        assert_eq!(s0.disk_total_kb, 10000);
        assert_eq!(s0.disk_used_kb, 4000);
        assert_eq!(s0.procs.len(), 1);
        assert_eq!(s0.procs[0].name, "firefox");

        let t2: Vec<String> = ["1100 950", "2000 500", "3000 5000", "10000 4000", "__P__"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let s1 = parse_tick(&t2, &mut prev).unwrap();
        assert!((s1.cpu - 50.0).abs() < 0.01, "cpu={}", s1.cpu);
        assert_eq!(s1.net_rx_bps, (3000 - 1000) / 2); // delta / interval
        assert_eq!(s1.net_tx_bps, (5000 - 2000) / 2);
    }
}
