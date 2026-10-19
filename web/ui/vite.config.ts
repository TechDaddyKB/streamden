import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Port 5173 is the Tauri devUrl (apps/desktop/tauri.conf.json); strictPort keeps them in sync.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022" },
});
