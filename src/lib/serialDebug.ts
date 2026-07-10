import { invoke, Channel } from "@tauri-apps/api/core";

/** Open a serial port for the debugger (raw bytes, no board reset on open). */
export async function openSerialRaw(
  path: string,
  baud: number,
  onBytes: (b: Uint8Array) => void,
): Promise<number> {
  const ch = new Channel<number[]>();
  ch.onmessage = (m) => onBytes(Uint8Array.from(m));
  return invoke<number>("serial_open", { path, baud, assertDtrRts: false, onOutput: ch });
}

export const writeSerialBytes = (id: number, data: number[]) =>
  invoke<void>("serial_write_bytes", { id, data });

export const closeSerial = (id: number) => invoke<void>("serial_close", { id });

export function fmtTime(t: number): string {
  const d = new Date(t);
  const p = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`;
}
