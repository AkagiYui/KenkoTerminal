import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { ptySpawn, ptyWrite, ptyResize, ptyKill } from "../lib/pty";

export function TerminalView() {
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
    const uni = new Unicode11Addon();
    term.loadAddon(uni);
    term.unicode.activeVersion = "11";
    term.open(el);
    try {
      term.loadAddon(new WebglAddon());
    } catch {
      /* WebGL unavailable — fall back to canvas/DOM renderer */
    }
    fit.fit();

    let ptyId: number | null = null;
    let disposed = false;

    void (async () => {
      const id = await ptySpawn(term.cols, term.rows, (bytes) => term.write(bytes));
      if (disposed) {
        void ptyKill(id);
        return;
      }
      ptyId = id;
      term.onData((d) => void ptyWrite(id, d));
    })();

    const ro = new ResizeObserver(() => {
      fit.fit();
      if (ptyId != null) void ptyResize(ptyId, term.cols, term.rows);
    });
    ro.observe(el);

    return () => {
      disposed = true;
      ro.disconnect();
      if (ptyId != null) void ptyKill(ptyId);
      term.dispose();
    };
  }, []);

  return <div ref={containerRef} className="h-full w-full" />;
}
