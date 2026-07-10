import { invoke, Channel } from "@tauri-apps/api/core";
import type { SshConfig } from "./session";

export type Proc = { cpu: number; name: string };
export type Sample = {
  cpu: number;
  mem_used_kb: number;
  mem_total_kb: number;
  disk_used_kb: number;
  disk_total_kb: number;
  net_rx_bps: number;
  net_tx_bps: number;
  procs: Proc[];
};
export type SystemInfo = { uname: string; os_release: string };

export const probeSystem = (config: SshConfig) => invoke<SystemInfo>("probe_system", { config });

export async function monitorStart(
  config: SshConfig,
  onSample: (s: Sample) => void,
): Promise<number> {
  const ch = new Channel<Sample>();
  ch.onmessage = onSample;
  return invoke<number>("monitor_start", { config, onSample: ch });
}

export const monitorStop = (id: number) => invoke<void>("monitor_stop", { id });
