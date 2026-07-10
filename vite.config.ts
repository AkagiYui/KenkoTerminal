import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import iconifyOffline from "vite-plugin-iconify-offline";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    react({
      babel: {
        // React Compiler — automatic memoization.
        plugins: [["babel-plugin-react-compiler", {}]],
      },
    }),
    tailwindcss(),
    iconifyOffline(),
  ],
  // Tauri expects a fixed dev port and quiet output.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
