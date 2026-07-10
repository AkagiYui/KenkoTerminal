import { useEffect, useRef, useState } from "react";
import {
  sftpConnect,
  sftpList,
  sftpRealpath,
  sftpRead,
  sftpWrite,
  sftpMkdir,
  sftpRemove,
  sftpRename,
  sftpClose,
  joinPath,
  parentPath,
  type FileEntry,
} from "../lib/sftp";

const inputCls =
  "w-full rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";
const iconBtn = "rounded px-1.5 py-0.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100";

/** Remote file manager. `cwd` (from the terminal via OSC 7) can be followed on demand. */
export function FileManager({ cwd }: { cwd?: string }) {
  const [id, setId] = useState<number | null>(null);
  const [path, setPath] = useState("/");
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [err, setErr] = useState("");
  const [host, setHost] = useState("");
  const [user, setUser] = useState("root");
  const [password, setPassword] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);
  const [follow, setFollow] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    return () => {
      if (id != null) void sftpClose(id);
    };
  }, [id]);

  // Auto-follow the terminal's cwd (OSC 7) when enabled.
  useEffect(() => {
    if (follow && id != null && cwd) void navigate(id, cwd);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cwd, follow, id]);

  async function navigate(cid: number, p: string) {
    try {
      const list = await sftpList(cid, p);
      setEntries(list);
      setPath(p);
      setErr("");
    } catch (e) {
      setErr(String(e));
    }
  }

  async function connect(e: React.FormEvent) {
    e.preventDefault();
    setErr("");
    try {
      const cid = await sftpConnect({ host, port: 22, user, password: password || undefined });
      setId(cid);
      const home = await sftpRealpath(cid, ".").catch(() => "/");
      await navigate(cid, home || "/");
    } catch (e) {
      setErr(String(e));
    }
  }

  const refresh = () => id != null && navigate(id, path);

  async function onEntry(entry: FileEntry) {
    if (id == null) return;
    if (entry.is_dir) return navigate(id, joinPath(path, entry.name));
    // download a file
    try {
      const bytes = await sftpRead(id, joinPath(path, entry.name));
      const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)]));
      const a = document.createElement("a");
      a.href = url;
      a.download = entry.name;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function upload(file: File) {
    if (id == null) return;
    setBusy(true);
    try {
      const buf = new Uint8Array(await file.arrayBuffer());
      await sftpWrite(id, joinPath(path, file.name), Array.from(buf));
      refresh();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function onDrop(e: React.DragEvent) {
    e.preventDefault();
    if (id == null) return;
    for (const f of Array.from(e.dataTransfer.files)) await upload(f);
  }

  async function mkdir() {
    if (id == null) return;
    const name = prompt("New folder name");
    if (!name) return;
    await sftpMkdir(id, joinPath(path, name)).catch((e) => setErr(String(e)));
    refresh();
  }

  async function remove(entry: FileEntry) {
    if (id == null) return;
    if (!confirm(`Delete ${entry.name}?`)) return;
    await sftpRemove(id, joinPath(path, entry.name), entry.is_dir).catch((e) => setErr(String(e)));
    refresh();
  }

  async function rename(entry: FileEntry) {
    if (id == null) return;
    const next = prompt("Rename to", entry.name);
    if (!next || next === entry.name) return;
    await sftpRename(id, joinPath(path, entry.name), joinPath(path, next)).catch((e) => setErr(String(e)));
    refresh();
  }

  if (id == null) {
    return (
      <form className="flex flex-col gap-2 p-3 text-sm" onSubmit={connect}>
        <div className="text-xs uppercase tracking-wider text-neutral-500">Files (SFTP)</div>
        <input className={inputCls} placeholder="host" value={host} onChange={(e) => setHost(e.target.value)} />
        <input className={inputCls} placeholder="user" value={user} onChange={(e) => setUser(e.target.value)} />
        <input className={inputCls} type="password" placeholder="password (or agent)" value={password} onChange={(e) => setPassword(e.target.value)} />
        <button className="rounded bg-teal-700 px-3 py-1.5 hover:bg-teal-600 disabled:opacity-40" disabled={!host}>
          Connect
        </button>
        {err && <div className="text-xs text-red-400">{err}</div>}
      </form>
    );
  }

  return (
    <div className="flex h-full flex-col text-sm">
      <div className="flex items-center gap-1 border-b border-neutral-800 px-2 py-1 text-xs">
        <button className={iconBtn} title="up" onClick={() => navigate(id, parentPath(path))}>
          ↑
        </button>
        <button className={iconBtn} title="refresh" onClick={refresh}>
          ↻
        </button>
        <button className={iconBtn} title="new folder" onClick={mkdir}>
          ＋
        </button>
        <button className={iconBtn} title="upload" onClick={() => fileInput.current?.click()}>
          ⤒
        </button>
        {cwd && (
          <button className={iconBtn} title={`go to terminal cwd: ${cwd}`} onClick={() => navigate(id, cwd)}>
            ⤓cwd
          </button>
        )}
        <button
          className={`${iconBtn} ${follow ? "text-teal-400" : ""}`}
          title="auto-follow terminal cwd (OSC 7)"
          onClick={() => setFollow((v) => !v)}
        >
          ⇄follow
        </button>
        {busy && <span className="ml-auto text-neutral-500">transferring…</span>}
        <input
          ref={fileInput}
          type="file"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void upload(f);
            e.target.value = "";
          }}
        />
      </div>
      <div className="truncate border-b border-neutral-800 px-2 py-1 text-xs text-neutral-400" title={path}>
        {path}
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto" onDragOver={(e) => e.preventDefault()} onDrop={onDrop}>
        {entries.map((entry) => (
          <li key={entry.name} className="group flex items-center gap-2 px-2 py-0.5 hover:bg-neutral-800/60">
            <button className="flex-1 truncate text-left" onClick={() => onEntry(entry)} title={entry.name}>
              {entry.is_dir ? "📁" : "📄"} {entry.name}
            </button>
            <span className="hidden shrink-0 items-center gap-1 text-xs text-neutral-500 group-hover:flex">
              <button className={iconBtn} title="rename" onClick={() => rename(entry)}>
                ✎
              </button>
              <button className={iconBtn} title="delete" onClick={() => remove(entry)}>
                🗑
              </button>
            </span>
          </li>
        ))}
        {entries.length === 0 && <li className="px-2 py-1 text-xs text-neutral-600">empty</li>}
      </ul>
      {err && <div className="border-t border-neutral-800 px-2 py-1 text-xs text-red-400">{err}</div>}
    </div>
  );
}
