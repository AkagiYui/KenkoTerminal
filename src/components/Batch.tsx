import { useState } from "react";
import { batchRun, type BatchResult } from "../lib/batch";

const inputCls =
  "rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";

/** Ansible-lite batch console (R13): run one command across a host group, recap per host. */
export function Batch({ host, user: u0, password: p0 }: { host?: string; user?: string; password?: string }) {
  const [hosts, setHosts] = useState(host ? `${host}\n` : "");
  const [user, setUser] = useState(u0 || "root");
  const [password, setPassword] = useState(p0 || "");
  const [command, setCommand] = useState("uname -a");
  const [results, setResults] = useState<BatchResult[]>([]);
  const [running, setRunning] = useState(false);

  const targets = hosts.split("\n").map((s) => s.trim()).filter(Boolean);

  async function run() {
    if (targets.length === 0 || !command) return;
    if (targets.length > 5 && !confirm(`Run on ${targets.length} hosts?`)) return;
    setResults([]);
    setRunning(true);
    try {
      await batchRun(
        targets.map((h) => ({
          label: h,
          ssh: { host: h, port: 22, user, password: password || undefined },
        })),
        command,
        (r) => setResults((prev) => [...prev, r]),
      );
    } finally {
      setRunning(false);
    }
  }

  const ok = results.filter((r) => r.ok).length;
  const fail = results.length - ok;

  return (
    <div className="flex h-full flex-col gap-2 p-3 text-sm">
      <div className="text-xs uppercase tracking-wider text-neutral-500">Batch exec — {targets.length} hosts</div>
      <div className="flex gap-2">
        <textarea
          className={`${inputCls} h-20 flex-1 font-mono text-xs`}
          placeholder="one host per line"
          value={hosts}
          onChange={(e) => setHosts(e.target.value)}
        />
        <div className="flex w-44 flex-col gap-1">
          <input className={inputCls} placeholder="user" value={user} onChange={(e) => setUser(e.target.value)} />
          <input
            className={inputCls}
            type="password"
            placeholder="password (or agent)"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>
      </div>
      <div className="flex gap-2">
        <input
          className={`${inputCls} flex-1 font-mono`}
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          placeholder="command"
          onKeyDown={(e) => {
            if (e.key === "Enter") void run();
          }}
        />
        <button
          className="rounded bg-teal-700 px-4 font-medium hover:bg-teal-600 disabled:opacity-40"
          disabled={running || targets.length === 0}
          onClick={run}
        >
          {running ? "Running…" : "Run"}
        </button>
      </div>
      <div className="text-xs text-neutral-400">
        <span className="text-teal-400">{ok} ok</span> · <span className="text-red-400">{fail} failed</span> ·{" "}
        {results.length}/{targets.length} done
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
        {results.map((r, i) => (
          <details key={i} className="rounded bg-neutral-900">
            <summary className="flex cursor-pointer items-center gap-2 px-2 py-1 text-xs">
              <span className={r.ok ? "text-teal-400" : "text-red-400"}>{r.ok ? "✓" : "✗"}</span>
              <span className="flex-1 truncate" title={r.label}>{r.label}</span>
              <span className="shrink-0 text-neutral-500">exit {r.exit_code} · {r.ms}ms</span>
            </summary>
            <pre className="max-h-40 overflow-auto whitespace-pre-wrap px-2 pb-2 text-[11px] text-neutral-300">
              {r.output}
            </pre>
          </details>
        ))}
      </div>
    </div>
  );
}
