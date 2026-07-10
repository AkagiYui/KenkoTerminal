import { useState } from "react";
import { TerminalView } from "./components/Terminal";
import { Tunnels } from "./components/Tunnels";
import { Serial } from "./components/Serial";
import { FileManager } from "./components/FileManager";
import { Monitor } from "./components/Monitor";
import { serialSetSignal, type Session, type SessionSpec } from "./lib/session";

const inputCls =
  "rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";
const btnCls = "rounded bg-teal-700 px-3 py-1.5 font-medium hover:bg-teal-600 active:bg-teal-800";
const chipCls = "rounded border border-neutral-700 px-2 py-0.5 text-xs hover:bg-neutral-800";

export default function App() {
  const [spec, setSpec] = useState<SessionSpec | null>(null);
  const [sessionKey, setSessionKey] = useState(0);
  const [status, setStatus] = useState("");
  const [active, setActive] = useState<Session | null>(null);
  const [dtr, setDtr] = useState(false);
  const [rts, setRts] = useState(false);
  const [showFiles, setShowFiles] = useState(false);
  const [showMonitor, setShowMonitor] = useState(false);
  const [cwd, setCwd] = useState<string | undefined>(undefined);

  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [user, setUser] = useState("root");
  const [password, setPassword] = useState("");

  function launch(next: SessionSpec) {
    setSpec(next);
    setSessionKey((k) => k + 1);
    setDtr(false);
    setRts(false);
  }

  async function setSignals(nextDtr: boolean, nextRts: boolean) {
    if (active?.kind !== "serial") return;
    setDtr(nextDtr);
    setRts(nextRts);
    await serialSetSignal(active.id, nextDtr, nextRts);
  }

  async function resetBoard() {
    if (active?.kind !== "serial") return;
    await serialSetSignal(active.id, false, true); // EN low
    setTimeout(() => void serialSetSignal(active.id, false, false), 150); // EN high → run
  }

  return (
    <div className="flex h-full w-full">
      <aside className="flex w-64 shrink-0 flex-col gap-4 overflow-y-auto border-r border-neutral-800 p-3 text-sm">
        <div className="text-xs font-semibold uppercase tracking-wider text-teal-400">KenkoTerminal</div>

        <button className={btnCls} onClick={() => launch({ kind: "local" })}>
          Local shell
        </button>

        <form
          className="flex flex-col gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            launch({
              kind: "ssh",
              config: { host, port: Number(port) || 22, user, password: password || undefined },
            });
          }}
        >
          <div className="text-xs uppercase tracking-wider text-neutral-500">SSH</div>
          <input className={inputCls} placeholder="host" value={host} onChange={(e) => setHost(e.target.value)} />
          <input className={inputCls} placeholder="port" value={port} onChange={(e) => setPort(e.target.value)} />
          <input className={inputCls} placeholder="user" value={user} onChange={(e) => setUser(e.target.value)} />
          <input
            className={inputCls}
            type="password"
            placeholder="password (blank = ssh-agent)"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <button className={btnCls} type="submit" disabled={!host}>
            Connect
          </button>
        </form>

        <div className="border-t border-neutral-800 pt-3">
          <Serial onLaunch={launch} />
        </div>

        <div className="border-t border-neutral-800 pt-3">
          <Tunnels />
        </div>
      </aside>

      <main className="flex min-h-0 flex-1 flex-col">
        <div className="flex h-8 items-center gap-2 border-b border-neutral-800 px-3 text-xs">
          <span className="truncate text-neutral-400" title={status}>
            {status || "idle"}
          </span>
          <div className="ml-auto flex items-center gap-1">
            {active?.kind === "serial" && (
              <>
                <button className={chipCls} onClick={resetBoard} title="pulse EN (reset to run)">
                  Reset
                </button>
                <button className={`${chipCls} ${dtr ? "bg-teal-800" : ""}`} onClick={() => setSignals(!dtr, rts)}>
                  DTR
                </button>
                <button className={`${chipCls} ${rts ? "bg-teal-800" : ""}`} onClick={() => setSignals(dtr, !rts)}>
                  RTS
                </button>
              </>
            )}
            <button
              className={`${chipCls} ${showMonitor ? "bg-teal-800" : ""}`}
              onClick={() => setShowMonitor((v) => !v)}
            >
              Monitor
            </button>
            <button
              className={`${chipCls} ${showFiles ? "bg-teal-800" : ""}`}
              onClick={() => setShowFiles((v) => !v)}
            >
              Files
            </button>
          </div>
        </div>

        <div className="flex min-h-0 flex-1">
          {showMonitor && (
            <div className="min-h-0 w-64 shrink-0 overflow-y-auto border-r border-neutral-800">
              <Monitor host={host} user={user} password={password} />
            </div>
          )}
          <div className="min-h-0 flex-1">
            {spec ? (
              <TerminalView
                key={sessionKey}
                spec={spec}
                onStatus={setStatus}
                onSession={setActive}
                onCwd={setCwd}
              />
            ) : (
              <div className="flex h-full items-center justify-center text-sm text-neutral-600">
                Start a local shell, SSH, or open a serial port
              </div>
            )}
          </div>
          {showFiles && (
            <div className="min-h-0 w-96 shrink-0 border-l border-neutral-800">
              <FileManager cwd={cwd} />
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
