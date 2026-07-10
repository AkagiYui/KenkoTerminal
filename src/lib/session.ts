import { invoke, Channel } from "@tauri-apps/api/core";

export type SshConfig = {
  host: string;
  port: number;
  user: string;
  password?: string;
};

export type SessionSpec = { kind: "local" } | { kind: "ssh"; config: SshConfig };

/** A live backend session (local PTY or SSH channel). */
export type Session = { kind: "local" | "ssh"; id: number };

/**
 * Spawn a session. Raw output bytes stream back over a Tauri channel.
 * Dispatches to the local-PTY or SSH backend by kind.
 */
export async function spawnSession(
  spec: SessionSpec,
  cols: number,
  rows: number,
  onOutput: (bytes: Uint8Array) => void,
): Promise<Session> {
  const channel = new Channel<number[]>();
  channel.onmessage = (msg) => onOutput(Uint8Array.from(msg));

  if (spec.kind === "local") {
    const id = await invoke<number>("pty_spawn", { cols, rows, onOutput: channel });
    return { kind: "local", id };
  }
  const id = await invoke<number>("ssh_connect", {
    config: spec.config,
    cols,
    rows,
    onOutput: channel,
  });
  return { kind: "ssh", id };
}

export function writeSession(s: Session, data: string): Promise<void> {
  return invoke(s.kind === "local" ? "pty_write" : "ssh_write", { id: s.id, data });
}

export function resizeSession(s: Session, cols: number, rows: number): Promise<void> {
  return invoke(s.kind === "local" ? "pty_resize" : "ssh_resize", { id: s.id, cols, rows });
}

export function killSession(s: Session): Promise<void> {
  return invoke(s.kind === "local" ? "pty_kill" : "ssh_close", { id: s.id });
}
