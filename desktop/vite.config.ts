// Vite-oppsett: alt bundles lokalt (ingen CDN). Fast port for Tauri `devUrl`.
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  test: { include: ["src/**/*.test.ts", "scripts/*.test.mjs"], environment: "node" },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
