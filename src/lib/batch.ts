import { invoke, Channel } from "@tauri-apps/api/core";
import type { SshConfig } from "./session";

export type BatchTarget = { label: string; ssh: SshConfig };
export type BatchResult = {
  label: string;
  host: string;
  ok: boolean;
  exit_code: number;
  output: string;
  ms: number;
};

/** Run a command across targets; results stream in as each host finishes. */
export async function batchRun(
  targets: BatchTarget[],
  command: string,
  onResult: (r: BatchResult) => void,
  forks?: number,
): Promise<void> {
  const ch = new Channel<BatchResult>();
  ch.onmessage = onResult;
  await invoke("batch_run", { targets, command, forks: forks ?? null, onResult: ch });
}
