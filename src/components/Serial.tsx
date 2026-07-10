import { useEffect, useState } from "react";
import { listSerialPorts, type SerialPortEntry, type SessionSpec } from "../lib/session";

const inputCls =
  "w-full rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";

/** Serial launcher: enumerate ports (with USB identity) and open one as a session. */
export function Serial({
  onLaunch,
  onSave,
}: {
  onLaunch: (s: SessionSpec) => void;
  onSave?: (path: string, baud: number) => void;
}) {
  const [ports, setPorts] = useState<SerialPortEntry[]>([]);
  const [path, setPath] = useState("");
  const [baud, setBaud] = useState("115200");

  const refresh = () =>
    listSerialPorts()
      .then((p) => {
        setPorts(p);
        setPath((cur) => cur || p[0]?.name || "");
      })
      .catch(() => {});

  useEffect(() => {
    refresh();
  }, []);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <div className="text-xs uppercase tracking-wider text-neutral-500">Serial</div>
        <button className="text-xs text-neutral-500 hover:text-neutral-300" onClick={refresh} title="rescan">
          ↻
        </button>
      </div>
      <select className={inputCls} value={path} onChange={(e) => setPath(e.target.value)}>
        {ports.length === 0 && <option value="">no ports</option>}
        {ports.map((p) => (
          <option key={p.name} value={p.name}>
            {p.name}
            {p.product ? ` — ${p.product}` : ""}
          </option>
        ))}
      </select>
      <div className="flex gap-1">
        <input className={inputCls} value={baud} onChange={(e) => setBaud(e.target.value)} placeholder="baud" />
        <button
          className="shrink-0 rounded bg-teal-700 px-3 py-1 text-xs hover:bg-teal-600 disabled:opacity-40"
          disabled={!path}
          onClick={() => onLaunch({ kind: "serial", path, baud: Number(baud) || 115200 })}
        >
          Open
        </button>
        {onSave && (
          <button
            className="shrink-0 rounded border border-neutral-700 px-2 py-1 text-xs hover:bg-neutral-800 disabled:opacity-40"
            disabled={!path}
            title="save connection"
            onClick={() => onSave(path, Number(baud) || 115200)}
          >
            ★
          </button>
        )}
      </div>
    </div>
  );
}
