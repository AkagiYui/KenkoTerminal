import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import {
  spawnSession,
  writeSession,
  resizeSession,
  killSession,
  type Session,
  type SessionSpec,
} from "../lib/session";

export function TerminalView({
  spec,
  onStatus,
  onSession,
}: {
  spec: SessionSpec;
  onStatus?: (s: string) => void;
  onSession?: (s: Session | null) => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const term = new Terminal({
      fontFamily: 'Menlo, Monaco, "Cascadia Mono", "Courier New", monospace',
      fontSize: 13,
      cursorBlink: true,
      allowProposedApi: true,
      theme: { background: "#0b0e14", foreground: "#e6e6e6" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    term.open(el);
    try {
      term.loadAddon(new WebglAddon());
    } catch {
      /* WebGL unavailable — fall back to canvas/DOM renderer */
    }
    fit.fit();

    let session: Session | null = null;
    let disposed = false;

    onStatus?.("connecting…");
    void (async () => {
      try {
        const s = await spawnSession(spec, term.cols, term.rows, (b) => term.write(b));
        if (disposed) {
          void killSession(s);
          return;
        }
        session = s;
        onStatus?.(`${spec.kind} connected`);
        onSession?.(s);
        term.onData((d) => void writeSession(s, d));
      } catch (e) {
        onStatus?.(`error: ${String(e)}`);
        term.writeln(`\r\n\x1b[31m${String(e)}\x1b[0m`);
      }
    })();

    const ro = new ResizeObserver(() => {
      fit.fit();
      if (session) void resizeSession(session, term.cols, term.rows);
    });
    ro.observe(el);

    return () => {
      disposed = true;
      ro.disconnect();
      if (session) void killSession(session);
      onSession?.(null);
      term.dispose();
    };
    // `spec` is fixed per mount — App remounts via `key` for a new session.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return <div ref={containerRef} className="h-full w-full" />;
}
