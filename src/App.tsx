import { useRef, useState } from "react";
import { TerminalView } from "./components/Terminal";
import { Tunnels } from "./components/Tunnels";
import { Serial } from "./components/Serial";
import { FileManager } from "./components/FileManager";
import { Monitor } from "./components/Monitor";
import { Batch } from "./components/Batch";
import { SerialDebugger } from "./components/SerialDebugger";
import { SavedConnections } from "./components/SavedConnections";
import { connSave } from "./lib/connections";
import { serialSetSignal, writeSession, type Session, type SessionSpec } from "./lib/session";
import { useTranslation } from "react-i18next";
import { Icon } from "@iconify/react";

const inputCls =
  "rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";
const btnCls = "rounded bg-teal-700 px-3 py-1.5 font-medium hover:bg-teal-600 active:bg-teal-800";
const chipCls = "rounded border border-neutral-700 px-2 py-0.5 text-xs hover:bg-neutral-800";

type Tab = { id: string; spec: SessionSpec; title: string };

function tabTitle(spec: SessionSpec): string {
  switch (spec.kind) {
    case "local":
      return "local";
    case "ssh":
      return spec.config.host;
    case "serial":
      return spec.path.split("/").pop() || "serial";
    case "telnet":
      return `${spec.host}:${spec.port}`;
  }
}

export default function App() {
  const { t, i18n } = useTranslation();
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);
  const [sessions, setSessions] = useState<Record<string, Session>>({});
  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;

  const [broadcast, setBroadcast] = useState(false);
  const [status, setStatus] = useState("");
  const [showFiles, setShowFiles] = useState(false);
  const [showMonitor, setShowMonitor] = useState(false);
  const [showBatch, setShowBatch] = useState(false);
  const [showDebugger, setShowDebugger] = useState(false);
  const [cwd, setCwd] = useState<string | undefined>(undefined);

  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [user, setUser] = useState("root");
  const [password, setPassword] = useState("");
  const [telnetHost, setTelnetHost] = useState("");
  const [telnetPort, setTelnetPort] = useState("23");
  const [connReload, setConnReload] = useState(0);

  async function saveSsh() {
    if (!host) return;
    await connSave(
      { id: crypto.randomUUID(), name: `${user}@${host}`, kind: "ssh", host, port: Number(port) || 22, user, has_password: !!password },
      password || "",
    );
    setConnReload((n) => n + 1);
  }
  async function saveTelnet() {
    if (!telnetHost) return;
    await connSave({ id: crypto.randomUUID(), name: `telnet ${telnetHost}:${telnetPort}`, kind: "telnet", host: telnetHost, port: Number(telnetPort) || 23, has_password: false });
    setConnReload((n) => n + 1);
  }
  async function saveSerial(path: string, baud: number) {
    await connSave({ id: crypto.randomUUID(), name: `serial ${path.split("/").pop()}`, kind: "serial", path, baud, has_password: false });
    setConnReload((n) => n + 1);
  }

  function launch(spec: SessionSpec) {
    const id = crypto.randomUUID();
    setTabs((t) => [...t, { id, spec, title: tabTitle(spec) }]);
    setActiveTab(id);
    setShowBatch(false);
    setShowDebugger(false);
  }

  function closeTab(id: string) {
    setTabs((t) => {
      const next = t.filter((x) => x.id !== id);
      setActiveTab((cur) => (cur === id ? next[next.length - 1]?.id ?? null : cur));
      return next;
    });
    setSessions((prev) => {
      const n = { ...prev };
      delete n[id];
      return n;
    });
  }

  function setTabSession(id: string, s: Session | null) {
    setSessions((prev) => {
      const n = { ...prev };
      if (s) n[id] = s;
      else delete n[id];
      return n;
    });
  }

  function handleInput(tabId: string, data: string) {
    if (broadcast) {
      for (const s of Object.values(sessionsRef.current)) void writeSession(s, data);
    } else {
      const s = sessionsRef.current[tabId];
      if (s) void writeSession(s, data);
    }
  }

  const active = activeTab ? sessions[activeTab] : null;
  const activeSpec = tabs.find((t) => t.id === activeTab)?.spec;
  const activeSshConfig = activeSpec?.kind === "ssh" ? activeSpec.config : undefined;
  const [dtr, setDtr] = useState(false);
  const [rts, setRts] = useState(false);
  async function setSignals(nd: boolean, nr: boolean) {
    if (active?.kind !== "serial") return;
    setDtr(nd);
    setRts(nr);
    await serialSetSignal(active.id, nd, nr);
  }
  async function resetBoard() {
    if (active?.kind !== "serial") return;
    await serialSetSignal(active.id, false, true);
    setTimeout(() => void serialSetSignal(active.id, false, false), 150);
  }

  return (
    <div className="flex h-full w-full">
      <aside className="flex w-64 shrink-0 flex-col gap-4 overflow-y-auto border-r border-neutral-800 p-3 text-sm">
        <div className="flex items-center gap-1 text-xs font-semibold uppercase tracking-wider text-teal-400">
          <Icon icon="lucide:terminal" className="text-base" /> KenkoTerminal
        </div>

        <SavedConnections onLaunch={launch} reload={connReload} />

        <button className={btnCls} onClick={() => launch({ kind: "local" })}>
          {t("localShell")}
        </button>

        <form
          className="flex flex-col gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            launch({ kind: "ssh", config: { host, port: Number(port) || 22, user, password: password || undefined } });
          }}
        >
          <div className="text-xs uppercase tracking-wider text-neutral-500">SSH</div>
          <input className={inputCls} placeholder="host" value={host} onChange={(e) => setHost(e.target.value)} />
          <input className={inputCls} placeholder="port" value={port} onChange={(e) => setPort(e.target.value)} />
          <input className={inputCls} placeholder="user" value={user} onChange={(e) => setUser(e.target.value)} />
          <input className={inputCls} type="password" placeholder="password (blank = ssh-agent)" value={password} onChange={(e) => setPassword(e.target.value)} />
          <div className="flex gap-1">
            <button className={`${btnCls} flex-1`} type="submit" disabled={!host}>Connect</button>
            <button type="button" className={chipCls} title="save connection" disabled={!host} onClick={saveSsh}>★</button>
          </div>
        </form>

        <div className="border-t border-neutral-800 pt-3">
          <Serial onLaunch={launch} onSave={saveSerial} />
        </div>

        <form
          className="flex flex-col gap-2 border-t border-neutral-800 pt-3"
          onSubmit={(e) => {
            e.preventDefault();
            launch({ kind: "telnet", host: telnetHost, port: Number(telnetPort) || 23 });
          }}
        >
          <div className="text-xs uppercase tracking-wider text-neutral-500">Telnet</div>
          <div className="flex gap-1">
            <input className={inputCls} placeholder="host" value={telnetHost} onChange={(e) => setTelnetHost(e.target.value)} />
            <input className={`${inputCls} w-16`} placeholder="port" value={telnetPort} onChange={(e) => setTelnetPort(e.target.value)} />
          </div>
          <div className="flex gap-1">
            <button className={`${btnCls} flex-1`} type="submit" disabled={!telnetHost}>Connect</button>
            <button type="button" className={chipCls} title="save connection" disabled={!telnetHost} onClick={saveTelnet}>★</button>
          </div>
        </form>

        <div className="border-t border-neutral-800 pt-3">
          <Tunnels />
        </div>
      </aside>

      <main className="flex min-h-0 flex-1 flex-col">
        <div className="flex h-8 items-center gap-2 border-b border-neutral-800 px-3 text-xs">
          <span className="truncate text-neutral-400" title={status}>{status || t("idle")}</span>
          <div className="ml-auto flex items-center gap-1">
            <button
              className={`${chipCls} ${broadcast ? "bg-amber-700 text-amber-50" : ""}`}
              onClick={() => setBroadcast((v) => !v)}
              title="mirror typed input to ALL open tabs"
            >
              ⛯ {t("broadcast")}
            </button>
            {active?.kind === "serial" && (
              <>
                <button className={chipCls} onClick={resetBoard} title="pulse EN (reset)">Reset</button>
                <button className={`${chipCls} ${dtr ? "bg-teal-800" : ""}`} onClick={() => setSignals(!dtr, rts)}>DTR</button>
                <button className={`${chipCls} ${rts ? "bg-teal-800" : ""}`} onClick={() => setSignals(dtr, !rts)}>RTS</button>
              </>
            )}
            <button className={`${chipCls} ${showDebugger ? "bg-teal-800" : ""}`} onClick={() => setShowDebugger((v) => !v)}>{t("debugger")}</button>
            <button className={`${chipCls} ${showBatch ? "bg-teal-800" : ""}`} onClick={() => setShowBatch((v) => !v)}>{t("batch")}</button>
            <button className={`${chipCls} ${showMonitor ? "bg-teal-800" : ""}`} onClick={() => setShowMonitor((v) => !v)}>{t("monitor")}</button>
            <button className={`${chipCls} ${showFiles ? "bg-teal-800" : ""}`} onClick={() => setShowFiles((v) => !v)}>{t("files")}</button>
            <button className={chipCls} onClick={() => i18n.changeLanguage(i18n.language === "zh" ? "en" : "zh")} title="language">
              {i18n.language === "zh" ? "EN" : "中"}
            </button>
          </div>
        </div>

        <div className="flex min-h-0 flex-1">
          {showMonitor && (
            <div className="min-h-0 w-64 shrink-0 overflow-y-auto border-r border-neutral-800">
              <Monitor
                host={activeSshConfig?.host ?? host}
                user={activeSshConfig?.user ?? user}
                password={activeSshConfig?.password ?? password}
              />
            </div>
          )}

          <div className="flex min-h-0 flex-1 flex-col">
            {showBatch ? (
              <Batch host={host} user={user} password={password} />
            ) : showDebugger ? (
              <SerialDebugger />
            ) : (
              <>
                {tabs.length > 0 && (
                  <div className="flex h-8 shrink-0 items-center gap-1 overflow-x-auto border-b border-neutral-800 px-2">
                    {tabs.map((t) => (
                      <div
                        key={t.id}
                        className={`flex shrink-0 items-center gap-1 rounded px-2 py-0.5 text-xs ${t.id === activeTab ? "bg-neutral-800 text-neutral-100" : "text-neutral-400 hover:bg-neutral-800/50"}`}
                      >
                        <button className="max-w-[10rem] truncate" onClick={() => setActiveTab(t.id)}>{t.title}</button>
                        <button className="text-neutral-500 hover:text-red-400" onClick={() => closeTab(t.id)}>×</button>
                      </div>
                    ))}
                  </div>
                )}
                <div className="relative min-h-0 flex-1">
                  {tabs.map((t) => (
                    <div key={t.id} className="absolute inset-0" style={{ display: t.id === activeTab ? "block" : "none" }}>
                      <TerminalView
                        spec={t.spec}
                        onStatus={t.id === activeTab ? setStatus : undefined}
                        onSession={(s) => setTabSession(t.id, s)}
                        onCwd={t.id === activeTab ? setCwd : undefined}
                        onInput={(d) => handleInput(t.id, d)}
                      />
                    </div>
                  ))}
                  {tabs.length === 0 && (
                    <div className="flex h-full items-center justify-center text-sm text-neutral-600">
                      {t("startHint")}
                    </div>
                  )}
                </div>
              </>
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
