# KenkoTerminal — 实现计划 (Implementation Plan)

> 一个 macOS / Windows 双平台的"懂集群、常驻后台"的终端瑞士军刀:
> 本地 / SSH / 串口 / Telnet 多协议终端 + 端口映射守护 + 无限重连 + 远程资源监控 +
> 自动跟踪目录的文件管理 + 串口调试器 + 批量执行(Ansible-lite)。

本文档由前期对 9 个同类开源项目(`tmp/` 下 CrabPort / Lumin-SSH / Netcatty / Termius / meatshell /
oxideterm / qssh / rssh / wezterm)的**代码级功能审计**总结而来。每一条实现都标注了"抄哪个项目"。

---

## 1. 产品范围 & 需求清单

| # | 需求 | 归属阶段 |
|---|---|---|
| R1 | 多协议终端:本地 shell / SSH / 串口(TTY) / Telnet | P0, P1, P3 |
| R2 | **复用本机密钥**:ssh-agent + OS 钥匙串,不自建一套 | P1 |
| R3 | 开机自启 | P2 |
| R4 | 启动后仅托盘图标,不显示主界面 | P2 |
| R5 | 自动启动端口映射(持久化隧道,启动即拉起) | P2 |
| R6 | **无限重连**(退避+抖动,网络切换/睡眠唤醒触发) | P2 |
| R7 | 远程系统探测(OS/arch/uptime) | P5 |
| R8 | 终端左侧栏显示系统资源占用(CPU/内存/磁盘/网络) | P5 |
| R9 | 文件管理(SFTP 浏览/上传/下载) | P4 |
| R10 | 文件管理**远端自动跟踪目录**(跟随终端 cwd) | P4 |
| R11 | 跨平台 macOS + Windows | 全程(P0 起 CI 双端) |
| R12 | **串口调试器**:HEX/ASCII、时间戳、发送栏、DTR/RTS/BREAK、按设备指纹重连、绘图器 | P3 |
| R13 | **批量执行(类 Ansible)**:广播输入 + 主机组并行点命令 + 结果汇总 | P6 |

---

## 2. 技术栈(已定)

### 后端 / 核心 —— **Rust + Tauri v2**
选型理由(见对话决策):Rust 的 SSH/串口/PTY 生态最契合且被 oxideterm/rssh 验证;Windows 本地 PTY(ConPTY)
Rust 成熟而 Go 是软肋(两个 Go 参考项目干脆没做本地 shell);Tauri v2 稳定,Wails v3 仍 alpha;
autostart/单实例/托盘/安全存储都有官方插件。**Perry 已排除**(它是原生控件、非 webview,且无终端控件)。

| 用途 | 选型 |
|---|---|
| 应用框架 | `tauri` v2 + 官方插件 `autostart` / `single-instance` / `positioner` / `updater` |
| 本地 PTY | `portable-pty`(Windows 走 ConPTY) |
| SSH / 转发 / exec | `russh`(+ `russh` keys)—— 进程内异步 |
| SFTP | `russh-sftp` |
| 串口 | `serialport`(枚举暴露 VID:PID:serial) |
| Telnet | 自写 TCP + IAC 状态机 |
| 密钥复用 | `keyring`(Keychain / Windows 凭据管理器)+ ssh-agent(`SSH_AUTH_SOCK` / Windows 命名管道) |
| 异步运行时 | `tokio` |
| 配置/清单存储 | `rusqlite`(hosts/groups/history)+ 保险库 `chacha20poly1305` + `argon2`,主密钥存 OS 钥匙串 |
| 本地文件监听/配置 | `notify` |

### 前端 —— **React 19 + React Compiler + Vite 8 + pnpm**(纯 SPA,无需 SSR)
选型已定。虽然 React 在合成基准上略逊 Solid/Svelte,但**本 App 的性能关键面(终端 xterm.js、图表 uPlot)
都绕开框架**,差异实测无感;换来的是最强的组件生态(Radix / Base UI / shadcn)与 AI 帮助。
**React Compiler** 自动记忆化,基本免手写 `useMemo`/`useCallback`,进一步抹平性能差。
同栈参考:**oxideterm(Rust + Tauri2 + React 19)**、Netcatty(Electron + React)。

| 用途 | 选型 |
|---|---|
| 基础库 | **React 19** + **React Compiler**(`babel-plugin-react-compiler`,经 `@vitejs/plugin-react` 接入) |
| 构建 / 包管理 | **Vite 8+** + **pnpm** |
| 终端渲染 | `@xterm/xterm` + addon `fit`/`webgl`/`unicode11`/`search`/`web-links`/`serialize`(框架无关) |
| CSS | Tailwind CSS v4 |
| 无头 / 样式组件 | Radix UI / Base UI / shadcn/ui |
| 图标 | **Iconify** —— `@iconify/react` + **`vite-plugin-iconify-offline`**(图标数据本地打包,桌面端**离线可用、无运行时 API 调用**) |
| 实时图表(监控+绘图器) | `uPlot`(框架无关) |
| 表格 + 虚拟滚动 | `@tanstack/react-table` + `@tanstack/react-virtual` |
| 可调分栏布局 | `react-resizable-panels`(后期可上 `dockview`) |
| 表单 | `react-hook-form` |
| 路由 | `react-router`(memory / hash 模式,Tauri SPA) |
| 状态 | React 内置 + 需要时 `zustand` / `jotai` |
| i18n | `react-i18next`(zh/en) |

---

## 3. 架构总纲

**核心原则(借鉴 wezterm mux):把"会话/隧道状态"放 Rust 核心,UI 是薄客户端。**
Rust 核心即使没有窗口也常驻存活 —— 这正是"托盘守护 + 关窗不掉线 + 开机自启拉隧道"的基础。

```
┌───────────────────────── Tauri App ─────────────────────────┐
│  Webview (React 19) —— 薄客户端,可开/关而不杀会话           │
│    terminal │ serial-debugger │ file-mgr │ monitor │ batch   │
│        ▲ events                   │ commands                 │
│        │        Tauri IPC(命令 + 事件 + 二进制快通道)       │
│        ▼                          ▼                          │
│  Rust Core(常驻,窗口无关)                                   │
│   ┌─────────┬─────────┬─────────┬─────────┬──────────────┐   │
│   │transport│  auth   │ tunnel  │ monitor │   batch      │   │
│   │local/ssh│agent/   │supervisor│probe/  │ fan-out exec │   │
│   │serial/  │keyring/ │+reconnect│/proc   │ +recap       │   │
│   │telnet   │knownhost│         │stream   │              │   │
│   ├─────────┴─────────┴─────────┴─────────┴──────────────┤   │
│   │ sftp(+OSC7 cwd)  │  config/vault  │  daemon(tray/    │   │
│   │                  │  (rusqlite)    │  autostart/hidden)│   │
│   └──────────────────┴────────────────┴──────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

### Rust 模块草案
```
src-tauri/src/
  transport/   trait Session { read/write/resize/close }; local.rs ssh.rs serial.rs telnet.rs
  auth/        agent.rs  keyring.rs  ssh_config.rs  known_hosts.rs
  tunnel/      supervisor.rs  forward_local.rs forward_remote.rs forward_dynamic.rs
  reconnect/   engine.rs(退避+抖动、无上限)  netwatch.rs(网络切换/睡眠唤醒)
  monitor/     probe.rs(uname/os-release)  collector.rs(/proc 流式 + 非 Linux 兜底)
  sftp/        ops.rs  cwd_track.rs(OSC7 + PROMPT_COMMAND 兜底)
  batch/       fanout.rs  recap.rs  fold.rs(相同输出折叠)
  config/      store.rs(rusqlite)  vault.rs  inventory.rs(hosts/groups)
  daemon/      tray.rs  autostart.rs  single_instance.rs  window.rs
  ipc/         commands.rs  events.rs  pty_channel.rs(二进制快通道)
```

---

## 4. 关键横切设计(最难的几处,先想清楚)

1. **无限重连引擎(R6)** —— 所有会话/隧道/串口共用一个受监督任务模型:
   - 退避 + 抖动(1s→30s 封顶),**不设最大次数**(9 个参考项目都设了上限,去掉即可)。
   - 触发源:TCP 断开、SSH keepalive 超时、**网络切换 / 睡眠唤醒事件**(mac `NWPathMonitor` / win 网络变更 —— 需平台 spike)。
   - **区分错误类型**:认证失败/密钥错 **不重试**(非瞬时错误);用户主动断开 → 暂停。
   - 参考结构:oxideterm `reconnectOrchestratorStore`、Termius `reconnectSaga`、qssh `AutoReconnect`(都要去掉次数上限)。

2. **密钥复用(R2)** —— 两条腿:
   - 认证优先 **ssh-agent**(读 `SSH_AUTH_SOCK`;Windows `\\.\pipe\openssh-ssh-agent`),russh 支持 agent 认证,天然支持 FIDO2。
   - 密码/私钥口令存 **OS 钥匙串**(`keyring`)。解析复用 `~/.ssh/config`、`known_hosts`、`IdentityFile`。
   - **host-key 校验别裸奔**:russh 的 `check_server_key` 必须实现(TOFU + 指纹固定),否则等于"全部接受"漏洞。
   - 参考:wezterm `ssh_agent.rs`、oxideterm `ssh/agent.rs` + `keychain.rs`、rssh `keyring_store.rs`。

3. **保险库 vs 开机自启的冲突** —— 自启隧道要在无人值守下取密钥,所以**加密主密钥必须放 OS 钥匙串(登录即解锁)**,
   不能藏在"手输主密码"后面,否则开机拉不起隧道。这是一个需明确的安全取舍(会削弱静态安全性)。

4. **自动跟踪目录(R10)** —— 首选 **OSC 7** 解析(从 PTY 流里读 `ESC]7;file://host/path`),兜底注入 `PROMPT_COMMAND`。
   要"文件变更也自动刷新"再加轮询或 inotify agent。参考:Netcatty `terminalCwdTracker`、meatshell OSC7、Lumin 的 hook、oxideterm 的远程 inotify agent。

5. **远程监控(R7/R8)** —— **复用一条长驻 SSH exec 通道**跑 shell 循环采样 `/proc/stat`+`meminfo`+`net/dev`+`df`,
   Rust 端算 CPU 增量差值。**非 Linux 远端**(macOS/BSD/Win)换 `top -l 1`/`vm_stat`/`Get-Counter`。面板隐藏时暂停采样。
   参考:meatshell `MON_CMD`、Lumin `dynamicProbeScript`、qssh `monitor_service.go`、Netcatty `sessionOps`。

6. **批量执行(R13)** —— **用 `exec` 通道,不用交互 PTY**。tokio 扇出 + 并发上限(= Ansible forks);
   逐主机 recap(ok/failed/unreachable/timeout)+ 相同输出折叠(clustershell `clush -b` 式)+ 重试失败子集。
   安全护栏:目标预览+确认、dry-run、分批灰度、审计日志。**不做幂等/playbook**(要就集成现成 ansible)。

7. **PTY 吞吐** —— 高吞吐终端输出别走 JSON 事件,用 Tauri 的 **channel / 自定义协议**做二进制快通道(经典坑)。

---

## 5. 分阶段路线图

> 原则:先搭"能跑的走骨架",再逐条竖切;把最高技术风险(Windows PTY + IPC 吞吐 + 守护层)最早验证。
> 每个阶段结束都应有**可运行、可演示**的成果。

### P0 — 骨架 & 打通(1 个 spike,消除最大风险)
- Tauri v2 + React 19 + Vite 8 + pnpm 脚手架(前端结构参考 `tmp/oxideterm`,Tauri 后端参考 `tmp/rssh`);启用 React Compiler。
- xterm.js ↔ 本地 PTY(`portable-pty`)用**二进制快通道**打通。
- **CI 矩阵从第一天起**:mac(arm64/x64)+ win(x64)出包。
- ✅ 成果:在 **mac 和 Windows** 上都能在 App 里敲本地 shell。(验证 ConPTY + IPC 吞吐)

### P1 — SSH 核心 + 终端 + 连接管理
- `russh`:连接 / 交互 shell / `check_server_key`(known_hosts TOFU)。
- 认证:ssh-agent + `~/.ssh/config` + 密钥文件;`keyring` 存密码。
- Rust 侧会话/标签模型,多会话并发;`transport::Session` trait 抽象。
- UI:连接管理器(增删改连接、主机**分组/标签**)+ 多标签终端。
- ✅ 成果:用你现有的 agent/密钥 SSH 上一台机器。

### P2 — 守护层(核心差异化,9 个项目都缺)
- 托盘图标 + **启动即隐藏** + **单实例** + **开机自启**(autostart 插件)。
- 窗口生命周期:Rust 核心无窗口常驻;托盘开/关窗;**关窗不掉会话**。
- mac Accessory(LSUIElement)/ win 不占任务栏。
- **隧道监督器**:持久化 L/R/D 规则,**启动即自动拉起**(抄 Netcatty `usePortForwardingAutoStart` + oxideterm 三种转发)。
- **无限重连引擎**:退避+抖动 + 网络切换/睡眠唤醒触发 + keepalive;作用于会话与隧道。
- ✅ 成果:开机 → 仅托盘 → 隧道自动开 → 拔网线/睡眠唤醒后自动恢复。

### P3 — 串口 + 串口调试器(R12)
- `serialport`:枚举(含 VID:PID:serial)、打开、波特率/校验位配置;Telnet 传输(小)。
- **两种模式**:串口终端(xterm)+ 调试器(HEX/ASCII 双列、行时间戳、发送栏 CR/LF/HEX、快捷发送宏)。
- **DTR/RTS/BREAK** 控制;**打开端口不复位**开关(默认不拉 DTR/RTS)。
- **按设备指纹重连** + 热插拔检测(mac IOKit / win WM_DEVICECHANGE / linux udev);**重连保留 scrollback**,打 `── board reset ──` 标记。
- 后续:串口绘图器(uPlot)、esptool 式自动复位时序。
- 参考:meatshell/rssh/oxideterm 的 serialport 用法(它们只当哑终端,调试器层要自建)。
- ✅ 成果:重启开发板 → 自动重连、保留启动日志、HEX 视图、DTR/RTS 可控。

### P4 — 文件管理 + 自动跟踪目录(R9/R10)
- `russh-sftp`:双栏浏览/上传/下载/mkdir/rename/删除、分块传输、拖拽上传。
- **OSC 7 解析**驱动文件面板跟随终端 cwd;`PROMPT_COMMAND` 兜底。
- 后续:远程 inotify agent 做实时变更(抄 oxideterm `agent/watcher.rs`)。
- ✅ 成果:文件面板自动跟着终端所在远程目录走。

### P5 — 系统监控侧栏(R7/R8)
- 远程探测(uname/os-release/arch/uptime)。
- **一条长驻 exec 通道**流式 `/proc` 采集 + CPU 差值;非 Linux 兜底。
- 左侧栏:uPlot 仪表(CPU/内存/磁盘/网络)+ 进程表(TanStack table/virtual);可调分栏。
- ✅ 成果:终端左侧实时显示该会话所在主机的资源占用。

### P6 — 批量 / 集群执行(R13,Ansible-lite)
- Tier 0:广播键盘输入到选中的多个已开终端(抄 oxideterm `broadcastStore`、meatshell、Termius)。
- Tier 1:对主机组并行 `exec` 点命令,tokio 扇出 + forks 上限;逐主机 recap + 相同输出折叠 + 重试失败子集。
- 安全:目标预览+确认、dry-run、分批灰度、审计日志。
- ✅ 成果:一条命令跑遍一个主机组,聚合看结果、一键重试失败。

### P7 — 加固 / 打包 / 打磨
- 保险库最终形态(主密钥入钥匙串,支撑自启);可选配置同步(WebDAV/S3/git)。
- **代码签名 + 公证**(mac notarization + win Authenticode)—— 自启应用不签名会被 Gatekeeper/SmartScreen 拦。
- 自动更新(`tauri-plugin-updater`);i18n(zh/en);暗色优先主题。
- 性能收尾:PTY 吞吐、监控采样按需暂停、内存。

---

## 6. "抄哪个项目"速查表

| 能力 | 主参考(`tmp/`) | 关键文件/符号 |
|---|---|---|
| 多协议后端结构 | oxideterm / rssh | `src-tauri/src/{local,ssh,serial}` |
| 本地 PTY(含 Win ConPTY) | oxideterm / rssh | `portable-pty`(**别学 CrabPort 的 Windows 空壳**) |
| ssh-agent 复用 | wezterm / oxideterm | `ssh_agent.rs` / `ssh/agent.rs` |
| OS 钥匙串 | rssh / oxideterm | `keyring_store.rs` / `keychain.rs` |
| 自启隧道 | **Netcatty** | `usePortForwardingAutoStart.ts`(唯一真正接通的) |
| L/R/D 转发 | oxideterm | `forwarding/{local,remote,dynamic}.rs` |
| 无限重连(去上限) | oxideterm / Termius / qssh | `reconnectOrchestratorStore` / `reconnectSaga` / `AutoReconnect` |
| 远程 /proc 监控 | meatshell / Lumin / qssh | `MON_CMD` / `dynamicProbeScript` / `monitor_service.go` |
| 监控侧栏 UI | Lumin / qssh | `ProbePanel` / dockview `MonitorPanel` |
| OSC7 目录跟踪 | Netcatty / meatshell / Termius | `terminalCwdTracker` / `CwdChanged` |
| 远程 inotify agent | oxideterm | `agent/src/watcher.rs` |
| 广播输入 | oxideterm / meatshell | `broadcastStore.ts` |
| 主机分组/清单 | CrabPort / qssh / oxideterm | `groups.rs` / `group_manager.go` |
| 跳板机/ProxyJump | rssh / Netcatty | `bastion.rs` |
| ZMODEM/trzsz | meatshell / oxideterm | `zmodem.rs` / `trzsz/` |
| 同栈整体参考 | **oxideterm** / rssh | oxideterm=Rust+Tauri2+**React19**+xterm 全套;rssh=Tauri 后端范式 |

---

## 7. 风险 & 注意事项(集中)

- **签名/公证是硬成本**:自启常驻应用必须做,否则装不上/被拦。提前排期。
- **host-key 校验**:`check_server_key` 必须实现,别写成"全部接受"。**agent 转发**对不可信主机做成按主机可选。
- **保险库 vs 自启**:主密钥必须入钥匙串才能无人值守拉隧道 —— 明确该取舍。
- **无限重连**:区分"网络断"(重试)与"认证失败"(不重试);处理睡眠唤醒/网络切换,别干等退避。
- **监控非 Linux**:`/proc` 只有 Linux 有;CPU% 要两次采样算差值;**只开一条长驻通道**,面板隐藏时暂停。
- **串口**:打开端口默认别拉 DTR/RTS(会复位板子);按 VID:PID:serial 重连,不认死路径;重连保留 scrollback。
- **批量执行**:别越过"跑命令 → 保证状态"那条线(那是 Ansible);大组操作必须有确认/dry-run/审计。
- **Windows 专项**:ConPTY、命名管道 ssh-agent、凭据管理器、注册表 Run 自启、无任务栏窗口。
- **WKWebView**:mac 是 Safari 内核,对个别新 CSS 特性滞后;两端都要测,别用太前沿的 CSS。
- **PTY 吞吐**:走二进制快通道,别用 JSON 事件。
- **许可**:参考项目只作功能借鉴,**Termius 是反编译商业闭源,绝不拷代码**;其余逐个看 license(GPL 不能进闭源)。

---

## 8. MVP 边界

**MVP = P0 + P1 + P2**:本地/SSH 多标签终端 + 复用本机密钥 + 托盘常驻 + 开机自启 + 自动端口映射 + 无限重连。
这已经是市面多数 SSH 客户端都没有的"常驻隧道守护 + 终端"。之后 P3(串口调试器)、P4(文件+跟踪目录)、
P5(监控侧栏)为高价值竖切,P6(批量)、P7(打磨)收尾。

---

## 9. 实现状态(已完成)

**P0–P6 + 需求 R1–R13 全部实现、逐阶段提交推送、mac+win 双平台 CI 绿**;SSH/转发/SFTP/监控/批量
已对真实服务器实测,串口已对烧录固件的 ESP32-C3 实测。

- 后端(Rust/Tauri2):`transport`(pty / ssh / serial / telnet)、`ssh`(agent+密码+known_hosts)、
  `tunnel`(自启 + 无限重连)、`monitor`(/proc 流式)、`sftp`、`batch`(fan-out)、`daemon`(托盘/自启/隐藏/单实例)。
- 前端(React19 + Vite8):多标签终端 + **广播**、串口调试器(Text/HEX/Plot)、文件管理(OSC7 跟踪)、
  监控侧栏、批量控制台。
- CI:`CI`(typecheck + cargo check,mac+win)+ `Bundle`(tauri build → 上传安装包 artifact)。

**唯一剩余 = P7 签名/公证/自动更新** —— 需你的 Apple/Windows 证书 + 更新签名密钥与托管端点,属硬前置,
非代码问题(详见 [README](README.md))。备好证书即可接上 CI 签名步骤与 `tauri-plugin-updater`。
