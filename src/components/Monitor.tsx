import { useEffect, useState } from "react";
import { monitorStart, monitorStop, probeSystem, type Sample, type SystemInfo } from "../lib/monitor";

const inputCls =
  "w-full rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";

function Bar({ pct, color }: { pct: number; color: string }) {
  return (
    <div className="h-2 w-full overflow-hidden rounded bg-neutral-800">
      <div className={`h-2 rounded ${color}`} style={{ width: `${Math.max(0, Math.min(100, pct))}%` }} />
    </div>
  );
}

/** Remote resource monitor: streams /proc over SSH and shows CPU/mem (R8). */
export function Monitor({ host: h0, user: u0, password: p0 }: { host?: string; user?: string; password?: string }) {
  const [id, setId] = useState<number | null>(null);
  const [host, setHost] = useState(h0 || "");
  const [user, setUser] = useState(u0 || "root");
  const [password, setPassword] = useState(p0 || "");
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [sample, setSample] = useState<Sample | null>(null);
  const [hist, setHist] = useState<number[]>([]);
  const [err, setErr] = useState("");

  useEffect(() => {
    return () => {
      if (id != null) void monitorStop(id);
    };
  }, [id]);

  async function connect(e: React.FormEvent) {
    e.preventDefault();
    setErr("");
    const cfg = { host, port: 22, user, password: password || undefined };
    try {
      void probeSystem(cfg).then(setInfo).catch(() => {});
      const mid = await monitorStart(cfg, (s) => {
        setSample(s);
        setHist((h) => [...h.slice(-59), s.cpu]);
      });
      setId(mid);
    } catch (e) {
      setErr(String(e));
    }
  }

  if (id == null) {
    return (
      <form className="flex flex-col gap-2 p-3 text-sm" onSubmit={connect}>
        <div className="text-xs uppercase tracking-wider text-neutral-500">Monitor</div>
        <input className={inputCls} placeholder="host" value={host} onChange={(e) => setHost(e.target.value)} />
        <input className={inputCls} placeholder="user" value={user} onChange={(e) => setUser(e.target.value)} />
        <input className={inputCls} type="password" placeholder="password (or agent)" value={password} onChange={(e) => setPassword(e.target.value)} />
        <button className="rounded bg-teal-700 px-3 py-1.5 hover:bg-teal-600 disabled:opacity-40" disabled={!host}>
          Start
        </button>
        {err && <div className="text-xs text-red-400">{err}</div>}
      </form>
    );
  }

  const memPct = sample && sample.mem_total_kb > 0 ? (sample.mem_used_kb / sample.mem_total_kb) * 100 : 0;
  const spark = hist.map((v, i) => `${(i / Math.max(hist.length - 1, 1)) * 120},${30 - (v / 100) * 30}`).join(" ");

  return (
    <div className="flex flex-col gap-3 p-3 text-sm">
      <div className="text-xs uppercase tracking-wider text-neutral-500">Monitor</div>
      {info && (
        <div className="rounded bg-neutral-900 p-2 text-[11px] leading-tight text-neutral-400">
          <div className="truncate" title={info.uname}>{info.uname}</div>
          <div className="truncate text-neutral-500" title={info.os_release}>{info.os_release}</div>
        </div>
      )}

      <div className="flex flex-col gap-1">
        <div className="flex justify-between text-xs">
          <span className="text-neutral-400">CPU</span>
          <span className="tabular-nums text-teal-400">{(sample?.cpu ?? 0).toFixed(1)}%</span>
        </div>
        <Bar pct={sample?.cpu ?? 0} color="bg-teal-500" />
        <svg viewBox="0 0 120 30" preserveAspectRatio="none" className="mt-1 h-8 w-full">
          <polyline fill="none" stroke="#2dd4bf" strokeWidth="1" points={spark} />
        </svg>
      </div>

      <div className="flex flex-col gap-1">
        <div className="flex justify-between text-xs">
          <span className="text-neutral-400">Memory</span>
          <span className="tabular-nums text-sky-400">
            {sample ? `${(sample.mem_used_kb / 1048576).toFixed(2)} / ${(sample.mem_total_kb / 1048576).toFixed(2)} GB` : "—"}
          </span>
        </div>
        <Bar pct={memPct} color="bg-sky-500" />
      </div>
    </div>
  );
}
