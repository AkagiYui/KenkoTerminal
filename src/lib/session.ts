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
  | { kind: "serial"; path: string; baud: number }
  | { kind: "telnet"; host: string; port: number };

export type SessionKind = "local" | "ssh" | "serial" | "telnet";

/** A live backend session. */
export type Session = { kind: SessionKind; id: number };

export type SerialPortEntry = {
  name: string;
  vid: number | null;
  pid: number | null;
  serial_number: string | null;
  product: string | null;
};

export const listSerialPorts = () => invoke<SerialPortEntry[]>("serial_list");

export const serialSetSignal = (id: number, dtr: boolean, rts: boolean) =>
  invoke<void>("serial_set_signal", { id, dtr, rts });

/** esptool-style reset: bootloader=false → run, true → ROM download mode. */
export const serialEspReset = (id: number, bootloader: boolean) =>
  invoke<void>("serial_esp_reset", { id, bootloader });

const WRITE: Record<SessionKind, string> = {
  local: "pty_write",
  ssh: "ssh_write",
  serial: "serial_write",
  telnet: "telnet_write",
};
const KILL: Record<SessionKind, string> = {
  local: "pty_kill",
  ssh: "ssh_close",
  serial: "serial_close",
  telnet: "telnet_close",
};

export async function spawnSession(
  spec: SessionSpec,
  cols: number,
  rows: number,
  onOutput: (bytes: Uint8Array) => void,
): Promise<Session> {
  const channel = new Channel<number[]>();
  channel.onmessage = (msg) => onOutput(Uint8Array.from(msg));

  switch (spec.kind) {
    case "local": {
      const id = await invoke<number>("pty_spawn", { cols, rows, onOutput: channel });
      return { kind: "local", id };
    }
    case "ssh": {
      const id = await invoke<number>("ssh_connect", { config: spec.config, cols, rows, onOutput: channel });
      return { kind: "ssh", id };
    }
    case "serial": {
      const id = await invoke<number>("serial_open", {
        path: spec.path,
        baud: spec.baud,
        assertDtrRts: false,
        onOutput: channel,
      });
      return { kind: "serial", id };
    }
    case "telnet": {
      const id = await invoke<number>("telnet_open", { host: spec.host, port: spec.port, onOutput: channel });
      return { kind: "telnet", id };
    }
  }
}

export function writeSession(s: Session, data: string): Promise<void> {
  return invoke(WRITE[s.kind], { id: s.id, data });
}

export function resizeSession(s: Session, cols: number, rows: number): Promise<void> {
  if (s.kind === "serial" || s.kind === "telnet") return Promise.resolve(); // no window size
  return invoke(s.kind === "local" ? "pty_resize" : "ssh_resize", { id: s.id, cols, rows });
}

export function killSession(s: Session): Promise<void> {
  return invoke(KILL[s.kind], { id: s.id });
}
