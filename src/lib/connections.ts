import { invoke } from "@tauri-apps/api/core";

export type Connection = {
  id: string;
  name: string;
  kind: "ssh" | "serial" | "telnet" | "local";
  host?: string | null;
  port?: number | null;
  user?: string | null;
  path?: string | null;
  baud?: number | null;
  has_password: boolean;
  group?: string | null;
};

export const connList = () => invoke<Connection[]>("conn_list");
export const connSave = (conn: Connection, password?: string) =>
  invoke<void>("conn_save", { conn, password: password ?? null });
export const connDelete = (id: string) => invoke<void>("conn_delete", { id });
export const connPassword = (id: string) => invoke<string | null>("conn_password", { id });
