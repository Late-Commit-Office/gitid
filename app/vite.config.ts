import { defineConfig } from "vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  // Tauri-specific settings, from Tauri's own template:
  // 1. don't hide Rust errors behind a cleared screen
  clearScreen: false,
  // 2. Tauri expects a fixed port; fail if it's taken
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    // 3. don't watch the Rust side
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // The webview is the system's own: WebView2 (Chromium) on Windows,
    // WebKit on macOS and Linux. Vite 8 defaults to Safari 16.4, which would
    // break macOS 10.15/11; target what those systems actually ship.
    target: ["chrome105", "safari13"],
  },
}));
