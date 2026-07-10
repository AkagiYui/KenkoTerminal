import { useState } from "react";
import { batchRun, type BatchResult } from "../lib/batch";
import { connList } from "../lib/connections";

const inputCls =
  "rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";
const chip = "rounded border border-neutral-700 px-2 py-0.5 text-xs hover:bg-neutral-800";

/** Ansible-lite batch console (R13): run one command across a host group, recap per host. */
export function Batch({ host, user: u0, password: p0 }: { host?: string; user?: string; password?: string }) {
  const [hosts, setHosts] = useState(host ? `${host}\n` : "");
  const [user, setUser] = useState(u0 || "root");
  const [password, setPassword] = useState(p0 || "");
  const [command, setCommand] = useState("uname -a");
  const [results, setResults] = useState<BatchResult[]>([]);
  const [running, setRunning] = useState(false);
  const [dryRun, setDryRun] = useState(false);
  const [fold, setFold] = useState(false);

  const targets = hosts.split("\n").map((s) => s.trim()).filter(Boolean);

  const buildTargets = (list: string[]) =>
    list.map((h) => ({ label: h, ssh: { host: h, port: 22, user, password: password || undefined } }));

  async function runOn(list: string[], replace: boolean) {
    if (list.length === 0 || !command) return;
    if (list.length > 5 && !confirm(`Run on ${list.length} hosts?`)) return;
    if (replace) setResults([]);
    setRunning(true);
    try {
      await batchRun(buildTargets(list), command, (r) =>
        setResults((prev) => [...prev.filter((x) => x.label !== r.label), r]),
      );
    } finally {
      setRunning(false);
    }
  }

  async function run() {
    if (dryRun) {
      setResults(
        targets.map((h) => ({ label: h, host: h, ok: true, exit_code: 0, output: "(dry-run — not executed)", ms: 0 })),
      );
      return;
    }
    await runOn(targets, true);
  }

  async function retryFailed() {
    const failed = results.filter((r) => !r.ok).map((r) => r.label);
    if (failed.length) await runOn(failed, false);
  }

  async function loadSaved() {
    const cs = await connList().catch(() => []);
    const sshHosts = cs.filter((c) => c.kind === "ssh" && c.host).map((c) => c.host as string);
    if (sshHosts.length) setHosts(`${[...new Set(sshHosts)].join("\n")}\n`);
  }

  const ok = results.filter((r) => r.ok).length;
  const fail = results.length - ok;
  const folded = Object.entries(
    results.reduce<Record<string, BatchResult[]>>((acc, r) => {
      (acc[r.output] ??= []).push(r);
      return acc;
    }, {}),
  );

  return (
    <div className="flex h-full flex-col gap-2 p-3 text-sm">
      <div className="flex items-center gap-2">
        <div className="text-xs uppercase tracking-wider text-neutral-500">Batch exec — {targets.length} hosts</div>
        <button className={chip} onClick={loadSaved} title="load hosts from saved SSH connections">load saved</button>
      </div>
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
      <div className="flex items-center gap-2">
        <input
          className={`${inputCls} flex-1 font-mono`}
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          placeholder="command"
          onKeyDown={(e) => e.key === "Enter" && run()}
        />
        <label className={`${chip} ${dryRun ? "bg-amber-800" : ""} cursor-pointer`}>
          <input type="checkbox" className="hidden" checked={dryRun} onChange={(e) => setDryRun(e.target.checked)} /> dry-run
        </label>
        <label className={`${chip} ${fold ? "bg-teal-800" : ""} cursor-pointer`}>
          <input type="checkbox" className="hidden" checked={fold} onChange={(e) => setFold(e.target.checked)} /> fold
        </label>
        <button
          className="rounded bg-teal-700 px-4 font-medium hover:bg-teal-600 disabled:opacity-40"
          disabled={running || targets.length === 0}
          onClick={run}
        >
          {running ? "Running…" : dryRun ? "Preview" : "Run"}
        </button>
      </div>
      <div className="flex items-center gap-3 text-xs text-neutral-400">
        <span className="text-teal-400">{ok} ok</span>
        <span className="text-red-400">{fail} failed</span>
        <span>{results.length}/{targets.length} done</span>
        {fail > 0 && !running && (
          <button className={`${chip} text-amber-300`} onClick={retryFailed}>retry failed ({fail})</button>
        )}
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
        {fold
          ? folded.map(([output, rs], i) => (
              <details key={i} className="rounded bg-neutral-900">
                <summary className="flex cursor-pointer items-center gap-2 px-2 py-1 text-xs">
                  <span className={rs[0].ok ? "text-teal-400" : "text-red-400"}>{rs[0].ok ? "✓" : "✗"}</span>
                  <span className="flex-1 truncate" title={rs.map((r) => r.label).join(", ")}>
                    {rs.length} host{rs.length > 1 ? "s" : ""}: {rs.map((r) => r.label).join(", ")}
                  </span>
                </summary>
                <pre className="max-h-40 overflow-auto whitespace-pre-wrap px-2 pb-2 text-[11px] text-neutral-300">{output}</pre>
              </details>
            ))
          : results.map((r, i) => (
              <details key={i} className="rounded bg-neutral-900">
                <summary className="flex cursor-pointer items-center gap-2 px-2 py-1 text-xs">
                  <span className={r.ok ? "text-teal-400" : "text-red-400"}>{r.ok ? "✓" : "✗"}</span>
                  <span className="flex-1 truncate" title={r.label}>{r.label}</span>
                  <span className="shrink-0 text-neutral-500">exit {r.exit_code} · {r.ms}ms</span>
                </summary>
                <pre className="max-h-40 overflow-auto whitespace-pre-wrap px-2 pb-2 text-[11px] text-neutral-300">{r.output}</pre>
              </details>
            ))}
      </div>
    </div>
  );
}
