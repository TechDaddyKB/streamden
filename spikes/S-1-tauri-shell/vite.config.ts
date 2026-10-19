import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

// Two pages: the main window and the always-on-top chat mock.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        chat: resolve(import.meta.dirname, "chat.html"),
      },
    },
  },
});
