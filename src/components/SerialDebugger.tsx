import { useEffect, useRef, useState } from "react";
import { listSerialPorts, serialSetSignal, serialEspReset, type SerialPortEntry } from "../lib/session";
import { openSerialRaw, writeSerialBytes, closeSerial, fmtTime } from "../lib/serialDebug";

const inputCls =
  "rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-neutral-100 outline-none focus:border-neutral-500";
const chip = "rounded border border-neutral-700 px-2 py-0.5 text-xs hover:bg-neutral-800";

type Line = { ts: number; text: string };
type View = "text" | "hex" | "plot";

function Spark({ data }: { data: number[] }) {
  if (data.length < 2) return <div className="h-8" />;
  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min || 1;
  const pts = data
    .map((v, i) => `${(i / (data.length - 1)) * 120},${28 - ((v - min) / range) * 26}`)
    .join(" ");
  return (
    <svg viewBox="0 0 120 30" preserveAspectRatio="none" className="h-8 w-full">
      <polyline fill="none" stroke="#2dd4bf" strokeWidth="1" points={pts} />
    </svg>
  );
}

/** Serial monitor / debugger (R12): Text / Hex / Plot views + byte-accurate send + DTR/RTS. */
export function SerialDebugger() {
  const [ports, setPorts] = useState<SerialPortEntry[]>([]);
  const [path, setPath] = useState("");
  const [baud, setBaud] = useState("115200");
  const [id, setId] = useState<number | null>(null);

  const [view, setView] = useState<View>("text");
  const [lines, setLines] = useState<Line[]>([]);
  const [bytes, setBytes] = useState<number[]>([]);
  const [series, setSeries] = useState<Record<string, number[]>>({});

  const [input, setInput] = useState("");
  const [ending, setEnding] = useState<"none" | "lf" | "crlf">("lf");
  const [hexMode, setHexMode] = useState(false);
  const [dtr, setDtr] = useState(false);
  const [rts, setRts] = useState(false);

  const pending = useRef("");
  const scroller = useRef<HTMLDivElement>(null);

  const refresh = () =>
    listSerialPorts().then((p) => {
      setPorts(p);
      setPath((cur) => cur || p[0]?.name || "");
    }).catch(() => {});
  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    return () => {
      if (id != null) void closeSerial(id);
    };
  }, [id]);

  useEffect(() => {
    scroller.current?.scrollTo(0, scroller.current.scrollHeight);
  }, [lines, bytes, view]);

  function onBytes(chunk: Uint8Array) {
    setBytes((prev) => [...prev, ...chunk].slice(-4096));
    pending.current += new TextDecoder("utf-8", { fatal: false }).decode(chunk);
    const parts = pending.current.split(/\r?\n/);
    pending.current = parts.pop() ?? "";
    if (parts.length === 0) return;
    const ts = Date.now();
    setLines((prev) => [...prev, ...parts.map((text) => ({ ts, text }))].slice(-1000));
    setSeries((prev) => {
      const next = { ...prev };
      for (const line of parts) {
        for (const m of line.matchAll(/([A-Za-z_]\w*)=(-?\d+(?:\.\d+)?)/g)) {
          const k = m[1];
          next[k] = [...(next[k] ?? []), parseFloat(m[2])].slice(-160);
        }
      }
      return next;
    });
  }

  async function open() {
    setLines([]);
    setBytes([]);
    setSeries({});
    pending.current = "";
    try {
      const sid = await openSerialRaw(path, Number(baud) || 115200, onBytes);
      setId(sid);
    } catch (e) {
      setLines([{ ts: Date.now(), text: `error: ${String(e)}` }]);
    }
  }

  async function close() {
    if (id != null) await closeSerial(id);
    setId(null);
  }

  function send() {
    if (id == null) return;
    let data: number[];
    if (hexMode) {
      data = input.trim().split(/[\s,]+/).filter(Boolean).map((h) => parseInt(h, 16) & 0xff);
    } else {
      const enc = Array.from(new TextEncoder().encode(input));
      const end = ending === "lf" ? [10] : ending === "crlf" ? [13, 10] : [];
      data = [...enc, ...end];
    }
    void writeSerialBytes(id, data);
    if (!hexMode) setInput("");
  }

  async function setSignals(nd: boolean, nr: boolean) {
    if (id == null) return;
    setDtr(nd);
    setRts(nr);
    await serialSetSignal(id, nd, nr);
  }
  async function reset() {
    if (id != null) await serialEspReset(id, false);
  }
  async function boot() {
    if (id != null) await serialEspReset(id, true);
  }

  const hexRows: { off: string; hex: string; ascii: string }[] = [];
  for (let i = Math.max(0, bytes.length - 640); i < bytes.length; i += 16) {
    const slice = bytes.slice(i, i + 16);
    hexRows.push({
      off: i.toString(16).padStart(6, "0"),
      hex: slice.map((b) => b.toString(16).padStart(2, "0")).join(" "),
      ascii: slice.map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : ".")).join(""),
    });
  }

  return (
    <div className="flex h-full flex-col gap-2 p-3 text-sm">
      <div className="flex flex-wrap items-center gap-2">
        <div className="text-xs uppercase tracking-wider text-neutral-500">Serial debugger</div>
        <select className={inputCls} value={path} onChange={(e) => setPath(e.target.value)} disabled={id != null}>
          {ports.length === 0 && <option value="">no ports</option>}
          {ports.map((p) => (
            <option key={p.name} value={p.name}>{p.name}{p.product ? ` — ${p.product}` : ""}</option>
          ))}
        </select>
        <input className={`${inputCls} w-24`} value={baud} onChange={(e) => setBaud(e.target.value)} disabled={id != null} />
        {id == null ? (
          <button className="rounded bg-teal-700 px-3 py-1 text-xs hover:bg-teal-600 disabled:opacity-40" disabled={!path} onClick={open}>Open</button>
        ) : (
          <button className="rounded bg-neutral-700 px-3 py-1 text-xs hover:bg-neutral-600" onClick={close}>Close</button>
        )}
        <button className={chip} onClick={refresh} title="rescan">↻</button>
        <div className="ml-auto flex items-center gap-1">
          {(["text", "hex", "plot"] as View[]).map((v) => (
            <button key={v} className={`${chip} ${view === v ? "bg-teal-800" : ""}`} onClick={() => setView(v)}>{v}</button>
          ))}
          {id != null && (
            <>
              <button className={chip} onClick={reset} title="reset to run">Reset</button>
              <button className={chip} onClick={boot} title="enter bootloader (esptool sequence)">Boot</button>
              <button className={`${chip} ${dtr ? "bg-teal-800" : ""}`} onClick={() => setSignals(!dtr, rts)}>DTR</button>
              <button className={`${chip} ${rts ? "bg-teal-800" : ""}`} onClick={() => setSignals(dtr, !rts)}>RTS</button>
            </>
          )}
        </div>
      </div>

      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto rounded bg-neutral-950 p-2 font-mono text-xs">
        {view === "text" &&
          lines.map((l, i) => (
            <div key={i} className="whitespace-pre-wrap">
              <span className="text-neutral-600">{fmtTime(l.ts)} </span>
              <span className="text-neutral-200">{l.text}</span>
            </div>
          ))}
        {view === "hex" &&
          hexRows.map((r, i) => (
            <div key={i} className="whitespace-pre text-neutral-300">
              <span className="text-neutral-600">{r.off}  </span>
              {r.hex.padEnd(47, " ")}  <span className="text-neutral-500">{r.ascii}</span>
            </div>
          ))}
        {view === "plot" && (
          <div className="flex flex-col gap-2">
            {Object.keys(series).length === 0 && <div className="text-neutral-600">no numeric key=value data yet</div>}
            {Object.entries(series).map(([k, data]) => (
              <div key={k}>
                <div className="flex justify-between text-[11px]">
                  <span className="text-teal-400">{k}</span>
                  <span className="tabular-nums text-neutral-400">{data[data.length - 1]}</span>
                </div>
                <Spark data={data} />
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center gap-1">
        <input
          className={`${inputCls} flex-1 font-mono`}
          placeholder={hexMode ? "hex bytes e.g. 70 69 6e 67" : "send text (e.g. ping)"}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && send()}
        />
        <select className={inputCls} value={ending} onChange={(e) => setEnding(e.target.value as typeof ending)} title="line ending">
          <option value="none">none</option>
          <option value="lf">LF</option>
          <option value="crlf">CRLF</option>
        </select>
        <label className={`${chip} ${hexMode ? "bg-teal-800" : ""} cursor-pointer`}>
          <input type="checkbox" className="hidden" checked={hexMode} onChange={(e) => setHexMode(e.target.checked)} /> HEX
        </label>
        <button className="rounded bg-teal-700 px-3 py-1 text-xs hover:bg-teal-600 disabled:opacity-40" disabled={id == null} onClick={send}>Send</button>
      </div>
    </div>
  );
}
