import { TerminalView } from "./components/Terminal";

export default function App() {
  return (
    <div className="flex h-full w-full flex-col">
      <header className="flex h-8 select-none items-center border-b border-neutral-800 px-3 text-xs text-neutral-400">
        KenkoTerminal — local shell
      </header>
      <main className="min-h-0 flex-1">
        <TerminalView />
      </main>
    </div>
  );
}
