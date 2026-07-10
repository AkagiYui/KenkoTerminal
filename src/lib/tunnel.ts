import { invoke } from "@tauri-apps/api/core";
import type { SshConfig } from "./session";

/** Matches the Rust `TunnelRule` (serde field names are snake_case). */
export type TunnelRule = {
  id: string;
  name: string;
  ssh: SshConfig;
  local_host: string;
  local_port: number;
  remote_host: string;
  remote_port: number;
  enabled: boolean;
};

export const listTunnels = () => invoke<TunnelRule[]>("tunnel_list");
export const addTunnel = (rule: TunnelRule) => invoke<void>("tunnel_add", { rule });
export const removeTunnel = (id: string) => invoke<void>("tunnel_remove", { id });
