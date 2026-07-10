import { useState } from "react";
import { TerminalView } from "./components/Terminal";
import { Tunnels } from "./components/Tunnels";
import type { SessionSpec } from "./lib/session";

const inputCls =
  "rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";
const btnCls = "rounded bg-teal-700 px-3 py-1.5 font-medium hover:bg-teal-600 active:bg-teal-800";

export default function App() {
  const [spec, setSpec] = useState<SessionSpec | null>(null);
  const [sessionKey, setSessionKey] = useState(0);
  const [status, setStatus] = useState("");

  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [user, setUser] = useState("root");
  const [password, setPassword] = useState("");

  function launch(next: SessionSpec) {
    setSpec(next);
    setSessionKey((k) => k + 1);
  }

  return (
    <div className="flex h-full w-full">
      <aside className="flex w-64 shrink-0 flex-col gap-4 border-r border-neutral-800 p-3 text-sm">
        <div className="text-xs font-semibold uppercase tracking-wider text-teal-400">
          KenkoTerminal
        </div>

        <button className={btnCls} onClick={() => launch({ kind: "local" })}>
          Local shell
        </button>

        <form
          className="flex flex-col gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            launch({
              kind: "ssh",
              config: {
                host,
                port: Number(port) || 22,
                user,
                password: password || undefined,
              },
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
            placeholder="password (blank = use ssh-agent)"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <button className={btnCls} type="submit" disabled={!host}>
            Connect
          </button>
        </form>

        <div className="overflow-y-auto border-t border-neutral-800 pt-3">
          <Tunnels />
        </div>

        <div className="mt-auto truncate text-xs text-neutral-500" title={status}>
          {status}
        </div>
      </aside>

      <main className="min-h-0 flex-1">
        {spec ? (
          <TerminalView key={sessionKey} spec={spec} onStatus={setStatus} />
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-neutral-600">
            Start a local shell or connect via SSH
          </div>
        )}
      </main>
    </div>
  );
}
