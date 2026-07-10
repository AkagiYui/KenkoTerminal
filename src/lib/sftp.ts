import { invoke } from "@tauri-apps/api/core";
import type { SshConfig } from "./session";

export type FileEntry = { name: string; is_dir: boolean; size: number };

export const sftpConnect = (config: SshConfig) => invoke<number>("sftp_connect", { config });
export const sftpList = (id: number, path: string) => invoke<FileEntry[]>("sftp_list", { id, path });
export const sftpRealpath = (id: number, path: string) => invoke<string>("sftp_realpath", { id, path });
export const sftpRead = (id: number, path: string) => invoke<number[]>("sftp_read", { id, path });
export const sftpWrite = (id: number, path: string, data: number[]) =>
  invoke<void>("sftp_write", { id, path, data });
export const sftpMkdir = (id: number, path: string) => invoke<void>("sftp_mkdir", { id, path });
export const sftpRemove = (id: number, path: string, isDir: boolean) =>
  invoke<void>("sftp_remove", { id, path, isDir });
export const sftpRename = (id: number, from: string, to: string) =>
  invoke<void>("sftp_rename", { id, from, to });
export const sftpClose = (id: number) => invoke<void>("sftp_close", { id });

export function joinPath(dir: string, name: string): string {
  return dir.endsWith("/") ? dir + name : `${dir}/${name}`;
}

export function parentPath(dir: string): string {
  const p = dir.replace(/\/+$/, "").replace(/\/[^/]*$/, "");
  return p === "" ? "/" : p;
}
