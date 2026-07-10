import { useEffect, useState } from "react";
import { listTunnels, addTunnel, removeTunnel, type TunnelRule } from "../lib/tunnel";

const inputCls =
  "w-full rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";

/** Port-forward manager. Rules persist and auto-start on launch (R5), reconnect forever (R6). */
export function Tunnels() {
  const [rules, setRules] = useState<TunnelRule[]>([]);
  const [host, setHost] = useState("");
  const [user, setUser] = useState("root");
  const [password, setPassword] = useState("");
  const [localPort, setLocalPort] = useState("");
  const [remoteHost, setRemoteHost] = useState("127.0.0.1");
  const [remotePort, setRemotePort] = useState("");
  const [mode, setMode] = useState("local");

  const refresh = () => listTunnels().then(setRules).catch(() => {});
  useEffect(() => {
    refresh();
  }, []);

  async function add(e: React.FormEvent) {
    e.preventDefault();
    const rule: TunnelRule = {
      id: crypto.randomUUID(),
      name:
        mode === "dynamic"
          ? `SOCKS :${localPort}`
          : mode === "remote"
            ? `-R ${remotePort} → :${localPort}`
            : `:${localPort} → ${remoteHost}:${remotePort}`,
      ssh: { host, port: 22, user, password: password || undefined },
      local_host: "127.0.0.1",
      local_port: Number(localPort),
      remote_host: remoteHost,
      remote_port: Number(remotePort) || 0,
      enabled: true,
      mode,
    };
    await addTunnel(rule);
    setLocalPort("");
    setRemotePort("");
    refresh();
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="text-xs uppercase tracking-wider text-neutral-500">Tunnels</div>

      <ul className="flex flex-col gap-1">
        {rules.map((r) => (
          <li key={r.id} className="flex items-center justify-between rounded bg-neutral-900 px-2 py-1 text-xs">
            <span className="truncate" title={`${r.ssh.user}@${r.ssh.host}  ${r.name}`}>
              <span className="mr-1 rounded bg-neutral-800 px-1 text-[10px] uppercase text-neutral-400">{r.mode || "local"}</span>
              {r.name}
            </span>
            <button
              className="ml-2 shrink-0 text-neutral-500 hover:text-red-400"
              onClick={() => removeTunnel(r.id).then(refresh)}
              title="remove"
            >
              ✕
            </button>
          </li>
        ))}
        {rules.length === 0 && <li className="text-xs text-neutral-600">no tunnels</li>}
      </ul>

      <form className="flex flex-col gap-1" onSubmit={add}>
        <input className={inputCls} placeholder="ssh host" value={host} onChange={(e) => setHost(e.target.value)} />
        <input className={inputCls} placeholder="ssh user" value={user} onChange={(e) => setUser(e.target.value)} />
        <input className={inputCls} type="password" placeholder="password (or agent)" value={password} onChange={(e) => setPassword(e.target.value)} />
        <select className={inputCls} value={mode} onChange={(e) => setMode(e.target.value)}>
          <option value="local">Local (-L)</option>
          <option value="remote">Remote (-R)</option>
          <option value="dynamic">Dynamic (SOCKS5)</option>
        </select>
        <div className="flex gap-1">
          <input className={inputCls} placeholder={mode === "remote" ? "local port" : "local port"} value={localPort} onChange={(e) => setLocalPort(e.target.value)} />
          {mode !== "dynamic" && (
            <>
              <input className={inputCls} placeholder={mode === "remote" ? "bind host" : "remote host"} value={remoteHost} onChange={(e) => setRemoteHost(e.target.value)} />
              <input className={inputCls} placeholder={mode === "remote" ? "server port" : "remote port"} value={remotePort} onChange={(e) => setRemotePort(e.target.value)} />
            </>
          )}
        </div>
        <button className="rounded bg-neutral-700 px-3 py-1 text-xs hover:bg-neutral-600" type="submit" disabled={!host || !localPort || (mode !== "dynamic" && !remotePort)}>
          Add {mode} forward
        </button>
      </form>
    </div>
  );
}
