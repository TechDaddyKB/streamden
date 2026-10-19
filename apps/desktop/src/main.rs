//! `streamden` desktop UI process (architecture §1, ADR-0002).
//!
//! Scaffold: opens one empty window. The engine + tray process, the graphics-compat ladder, the
//! first-paint watchdog and web-process termination on close are specified in spec 07.

// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        // Must be the first plugin: it creates the GL context before WebKitGTK does. It only acts on
        // Wayland with an NVIDIA GPU, so it sets nothing on other systems.
        .plugin(tauri_plugin_wayland_nvidia_quirk::init())
        .run(tauri::generate_context!())
        .expect("failed to run the StreamDen UI");
}
