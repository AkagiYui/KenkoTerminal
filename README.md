# KenkoTerminal

A macOS / Windows "always-on" terminal Swiss-army-knife: multi-protocol terminal
(local / SSH / serial / Telnet) + a tray daemon that auto-starts port-forwards and
reconnects forever, plus a remote resource monitor, an auto-tracking SFTP file
manager, a serial debugger, and Ansible-lite batch execution.

Built with **Rust + Tauri 2** (backend) and **React 19 + Vite 8 + Tailwind** (UI).
See [`PLAN.md`](PLAN.md) for the full design, and the per-project audit it was
distilled from.

## Features

| # | Feature | Status |
|---|---------|--------|
| R1 | Multi-protocol terminal: local shell / SSH / serial / Telnet | ✅ |
| R2 | Reuse native keys — **ssh-agent** (`SSH_AUTH_SOCK` / Windows named pipe) + password | ✅ |
| R3 | Start at login (autostart) | ✅ |
| R4 | Tray icon, start hidden (no main window; macOS Accessory) | ✅ |
| R5 | Auto-start persisted port-forwards on launch | ✅ |
| R6 | Infinite reconnect (capped exponential backoff + liveness probe) | ✅ |
| R7 | Remote system probe (uname / os-release / uptime) | ✅ |
| R8 | Resource monitor sidebar (`/proc` CPU/mem, streamed over one exec channel) | ✅ |
| R9 | SFTP file manager (browse / up / down / mkdir / rename / delete) | ✅ |
| R10 | Remote directory auto-tracking via OSC 7 | ✅ |
| R11 | Cross-platform macOS + Windows (CI-green on both) | ✅ |
| R12 | Serial debugger: Text / Hex / Plot, timestamps, byte/hex send, DTR/RTS/Reset | ✅ |
| R13 | Batch exec (Tier-1 fan-out + recap) & broadcast to tabs (Tier-0) | ✅ |
| P7 | Code signing / notarization / auto-update | ⛔ needs your certs (see below) |

## Develop / run

Prereqs: Rust (stable), Node 20+, pnpm, and platform build tools (macOS: Xcode CLT;
Windows: WebView2, bundled).

```bash
pnpm install
pnpm tauri dev        # launches the app (tray-only; click the tray → Show)
pnpm tauri build      # produce release bundles into src-tauri/target/release/bundle
```

> On launch the app shows **only a tray icon** (by design — R3/R4). Click it → *Show*.

Disk note: Rust `target/` is the big consumer; the dev profile already trims debug
info (`Cargo.toml [profile.dev]`). `cargo clean` in `src-tauri/` when it balloons.

## Prebuilt downloads

Every push to `main` runs the **Bundle** workflow and uploads installers as
artifacts (`kenkoterminal-macos` = dmg/app, `kenkoterminal-windows` = nsis/msi):

```bash
gh run download --repo AkagiYui/KenkoTerminal -n kenkoterminal-macos   # latest
```

Or GitHub → **Actions → Bundle → (run) → Artifacts**. Bundles are **unsigned**:
- macOS: `xattr -cr KenkoTerminal.app` (or right-click → Open).
- Windows: SmartScreen → More info → Run anyway.

## Tests

Pure-logic unit tests (backoff, telnet IAC, /proc parse) run in CI. Live-server smoke
tests are opt-in via env (no credentials in the repo):

```bash
KENKO_SSH_HOST=... KENKO_SSH_USER=... KENKO_SSH_PASS=... \
  cargo test --manifest-path src-tauri/Cargo.toml
```

## ESP32-C3 test firmware

A serial-test firmware (115200; banner + `tick=/sine=/saw=/heap=` telemetry, ideal
for the serial plotter; `ping`→`pong` echo) is built by the ESP-IDF pipeline and
downloaded to `tmp/esp32c3-fw/` (git-ignored). Flash locally with esptool:

```bash
esptool.py --chip esp32c3 -p <PORT> write_flash 0x0 kenko-c3-merged.bin
```

## Remaining (needs your infrastructure)

**P7 — code signing / notarization / auto-update** is the only outstanding work and
cannot be done without your credentials:
- macOS: Apple Developer ID cert + notarization.
- Windows: Authenticode code-signing cert.
- Auto-update (`tauri-plugin-updater`): an update-signing keypair + a hosting endpoint.

Provide these and the CI signing steps + updater config can be wired in.
