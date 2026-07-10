import { useEffect, useState } from "react";
import { connList, connDelete, connPassword, type Connection } from "../lib/connections";
import type { SessionSpec } from "../lib/session";

/** Saved connections (passwords come from the OS keychain on launch). */
export function SavedConnections({
  onLaunch,
  reload,
}: {
  onLaunch: (s: SessionSpec) => void;
  reload: number;
}) {
  const [conns, setConns] = useState<Connection[]>([]);

  const refresh = () => connList().then(setConns).catch(() => {});
  useEffect(() => {
    refresh();
  }, [reload]);

  async function launch(c: Connection) {
    let spec: SessionSpec;
    if (c.kind === "ssh") {
      const password = c.has_password ? (await connPassword(c.id)) ?? undefined : undefined;
      spec = { kind: "ssh", config: { host: c.host || "", port: c.port || 22, user: c.user || "root", password } };
    } else if (c.kind === "serial") {
      spec = { kind: "serial", path: c.path || "", baud: c.baud || 115200 };
    } else if (c.kind === "telnet") {
      spec = { kind: "telnet", host: c.host || "", port: c.port || 23 };
    } else {
      spec = { kind: "local" };
    }
    onLaunch(spec);
  }

  async function del(id: string, e: React.MouseEvent) {
    e.stopPropagation();
    await connDelete(id);
    refresh();
  }

  if (conns.length === 0) return null;

  return (
    <div className="flex flex-col gap-1">
      <div className="text-xs uppercase tracking-wider text-neutral-500">Saved</div>
      {conns.map((c) => (
        <div
          key={c.id}
          className="group flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-xs hover:bg-neutral-800"
          onClick={() => launch(c)}
          title={`launch ${c.name}`}
        >
          <span className="w-10 shrink-0 text-neutral-500">{c.kind}</span>
          <span className="flex-1 truncate">{c.name}</span>
          <button className="hidden text-neutral-500 hover:text-red-400 group-hover:block" onClick={(e) => del(c.id, e)}>
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
