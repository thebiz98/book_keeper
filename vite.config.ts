/// <reference types="vitest" />
import { defineConfig } from "vite";
import { configDefaults } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "node:path";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Only the frontend is served from here. Rust sources and cargo output (target/ is GBs and is
    // rewritten on every build) and agent worktrees must not be watched: they made the dev server
    // burn CPU and disk.
    watch: {
      ignored: ["**/src-tauri/**", "**/target/**", "**/.claude/**", "**/dist/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  test: {
    environment: "happy-dom",
    setupFiles: ["./src/__tests__/setup.ts"],
    exclude: [...configDefaults.exclude, "**/.claude/**", "**/target/**"],
  },
  build: {
    target: process.env.TAURI_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
