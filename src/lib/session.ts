import { invoke, Channel } from "@tauri-apps/api/core";

export type SshConfig = {
  host: string;
  port: number;
  user: string;
  password?: string;
};

export type SessionSpec =
  | { kind: "local" }
  | { kind: "ssh"; config: SshConfig }
  | { kind: "serial"; path: string; baud: number };

/** A live backend session (local PTY, SSH channel, or serial port). */
export type Session = { kind: "local" | "ssh" | "serial"; id: number };

export type SerialPortEntry = {
  name: string;
  vid: number | null;
  pid: number | null;
  serial_number: string | null;
  product: string | null;
};

export const listSerialPorts = () => invoke<SerialPortEntry[]>("serial_list");

/** DTR/RTS line control (board reset / boot-mode). */
export const serialSetSignal = (id: number, dtr: boolean, rts: boolean) =>
  invoke<void>("serial_set_signal", { id, dtr, rts });

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
  if (spec.kind === "ssh") {
    const id = await invoke<number>("ssh_connect", {
      config: spec.config,
      cols,
      rows,
      onOutput: channel,
    });
    return { kind: "ssh", id };
  }
  const id = await invoke<number>("serial_open", {
    path: spec.path,
    baud: spec.baud,
    assertDtrRts: false, // never auto-reset the board on connect
    onOutput: channel,
  });
  return { kind: "serial", id };
}

export function writeSession(s: Session, data: string): Promise<void> {
  const cmd = s.kind === "local" ? "pty_write" : s.kind === "ssh" ? "ssh_write" : "serial_write";
  return invoke(cmd, { id: s.id, data });
}

export function resizeSession(s: Session, cols: number, rows: number): Promise<void> {
  if (s.kind === "serial") return Promise.resolve(); // serial has no window size
  const cmd = s.kind === "local" ? "pty_resize" : "ssh_resize";
  return invoke(cmd, { id: s.id, cols, rows });
}

export function killSession(s: Session): Promise<void> {
  const cmd = s.kind === "local" ? "pty_kill" : s.kind === "ssh" ? "ssh_close" : "serial_close";
  return invoke(cmd, { id: s.id });
}
