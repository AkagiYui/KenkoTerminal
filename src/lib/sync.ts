import { invoke } from "@tauri-apps/api/core";

export const configSyncPush = (url: string, user: string, password: string) =>
  invoke<void>("config_sync_push", { url, user, password });
export const configSyncPull = (url: string, user: string, password: string) =>
  invoke<void>("config_sync_pull", { url, user, password });
