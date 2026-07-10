import { useState } from "react";
import { configSyncPush, configSyncPull } from "../lib/sync";

const inputCls =
  "w-full rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";

/** Config sync (WebDAV/HTTP). Passwords stay in the keychain and are not synced. */
export function SyncPanel() {
  const [url, setUrl] = useState(() => localStorage.getItem("sync.url") || "");
  const [user, setUser] = useState(() => localStorage.getItem("sync.user") || "");
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState("");

  function persist() {
    localStorage.setItem("sync.url", url);
    localStorage.setItem("sync.user", user);
  }
  async function push() {
    persist();
    setStatus("pushing…");
    try {
      await configSyncPush(url, user, password);
      setStatus("pushed ✓");
    } catch (e) {
      setStatus(String(e));
    }
  }
  async function pull() {
    persist();
    setStatus("pulling…");
    try {
      await configSyncPull(url, user, password);
      setStatus("pulled ✓ (reopen panels)");
    } catch (e) {
      setStatus(String(e));
    }
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="text-xs uppercase tracking-wider text-neutral-500">Config sync</div>
      <input className={inputCls} placeholder="WebDAV URL (…/kenko.json)" value={url} onChange={(e) => setUrl(e.target.value)} />
      <div className="flex gap-1">
        <input className={inputCls} placeholder="user" value={user} onChange={(e) => setUser(e.target.value)} />
        <input className={inputCls} type="password" placeholder="pass" value={password} onChange={(e) => setPassword(e.target.value)} />
      </div>
      <div className="flex gap-1">
        <button className="flex-1 rounded bg-neutral-700 px-2 py-1 text-xs hover:bg-neutral-600 disabled:opacity-40" disabled={!url} onClick={push}>Push</button>
        <button className="flex-1 rounded bg-neutral-700 px-2 py-1 text-xs hover:bg-neutral-600 disabled:opacity-40" disabled={!url} onClick={pull}>Pull</button>
      </div>
      {status && <div className="truncate text-[11px] text-neutral-500" title={status}>{status}</div>}
    </div>
  );
}
