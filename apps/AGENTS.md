# apps/ — agent notes

Binaries. Root `AGENTS.md` still applies.

- `apps/daemon` → `streamdend`: the engine without a tray or webview (architecture §1). Depends on `den-core` and
  `den-server`.
- `apps/desktop` → `streamden`: the Tauri 2 **UI process**, kept thin. It loads `web/ui` and talks to the engine
  over the public local API like any third-party client (D-006). It must never depend on `den-core` or
  `den-server`: the always-running engine + tray process never loads WebKit (ADR-0002, S-1 condition 5). Where
  the engine + tray binary lives and how the UI is launched is decided in spec 07.
- Spec 07 also owns the graphics-compat ladder, the first-paint watchdog, broken-driver detection and terminating
  the web process when a window closes (ADR-0002 conditions 1–5). Until then the only compat measure is the
  `tauri-plugin-wayland-nvidia-quirk` plugin (S-1 level 1), which acts only on Wayland + NVIDIA.
- **Never set WebKit/GPU environment variables unconditionally** (`WEBKIT_DISABLE_*`, `GDK_BACKEND`,
  `__NV_*`). They belong to the graphics-compat setting.
- No WebGL in the main UI. No private UI-only commands or endpoints; there are no custom Tauri commands.
- Icons in `apps/desktop/icons` are placeholders from spike S-1; `icon.ico` keeps the Windows build working.

Checks: `cargo build --workspace`; `cargo tauri dev` from `apps/desktop` (starts the `web/ui` Vite server on
port 5173 and opens an empty window). A plain `cargo build` of `streamden` is a dev build that loads the Vite
URL; `cargo tauri build` from `apps/desktop` builds `web/ui` and embeds it (`custom-protocol`, release).
