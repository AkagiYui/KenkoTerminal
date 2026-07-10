import i18n from "i18next";
import { initReactI18next } from "react-i18next";

const en = {
  localShell: "Local shell",
  connect: "Connect",
  ssh: "SSH",
  telnet: "Telnet",
  serial: "Serial",
  tunnels: "Tunnels",
  saved: "Saved",
  monitor: "Monitor",
  files: "Files",
  batch: "Batch",
  debugger: "Debugger",
  broadcast: "Broadcast",
  idle: "idle",
  startHint: "Start a local shell, SSH, serial, or telnet",
  host: "host",
  port: "port",
  user: "user",
  passwordAgent: "password (blank = ssh-agent)",
};

const zh: typeof en = {
  localShell: "本地终端",
  connect: "连接",
  ssh: "SSH",
  telnet: "Telnet",
  serial: "串口",
  tunnels: "隧道",
  saved: "已保存",
  monitor: "监控",
  files: "文件",
  batch: "批量",
  debugger: "调试器",
  broadcast: "广播",
  idle: "空闲",
  startHint: "打开本地终端 / SSH / 串口 / Telnet",
  host: "主机",
  port: "端口",
  user: "用户",
  passwordAgent: "密码(留空 = 用 ssh-agent)",
};

const prefersZh = typeof navigator !== "undefined" && navigator.language.startsWith("zh");

i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, zh: { translation: zh } },
  lng: prefersZh ? "zh" : "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
