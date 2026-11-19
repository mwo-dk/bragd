import { defineConfig, searchForWorkspaceRoot } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// No svelte.config.js: the defaults are what we want (configFile: false keeps the build quiet).
// Tauri expects a fixed port and does not want the screen cleared. The splash, the runic font
// and the sprite sheets come from ../assets and src/lib.
export default defineConfig({
  plugins: [svelte({ configFile: false })],
  clearScreen: false,
  server: { port: 1420, strictPort: true, fs: { allow: [searchForWorkspaceRoot(process.cwd()), "../assets"] } },
  build: { target: "es2022" },
});
