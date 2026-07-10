import { invoke, Channel } from "@tauri-apps/api/core";

export type PtyId = number;

/**
 * Spawn a local PTY. Raw output bytes stream back over a Tauri channel.
 * (P0 sends Vec<u8> as a number[]; a binary fast-path is a later perf task.)
 */
export async function ptySpawn(
  cols: number,
  rows: number,
  onOutput: (bytes: Uint8Array) => void,
): Promise<PtyId> {
  const channel = new Channel<number[]>();
  channel.onmessage = (msg) => onOutput(Uint8Array.from(msg));
  return await invoke<PtyId>("pty_spawn", { cols, rows, onOutput: channel });
}

export function ptyWrite(id: PtyId, data: string): Promise<void> {
  return invoke("pty_write", { id, data });
}

export function ptyResize(id: PtyId, cols: number, rows: number): Promise<void> {
  return invoke("pty_resize", { id, cols, rows });
}

export function ptyKill(id: PtyId): Promise<void> {
  return invoke("pty_kill", { id });
}
