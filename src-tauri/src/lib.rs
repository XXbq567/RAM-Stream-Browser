/// RAM-Stream-Browser: Crate root.
/// Sprint 1: Tauri shell + Webview2 → loads B站, extracts cookies & __playinfo__.

pub mod adapters;
pub mod commands;
pub mod error;
pub mod ipc;
pub mod mpv;
pub mod rules;
pub mod updater;

use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;

/// Preload script injected into every page (including B站 after navigation).
/// Runs before page scripts, stays active across SPA navigation.
const PRELOAD_SCRIPT: &str = include_str!("../../src/inject/preload.js");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Create window programmatically so we can inject the preload script.
            // The init script runs on every page load, even after navigating to B站.
            let _window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("RAM-Stream-Browser")
                .inner_size(1600.0, 900.0)
                .resizable(true)
                .initialization_script(PRELOAD_SCRIPT)
                .build()?;

            log::info!("[RAM-Stream] v{} — Window created with preload script ({} bytes)",
                app.package_info().version,
                PRELOAD_SCRIPT.len()
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::cookie::submit_cookies,
            commands::stream::submit_playinfo,
            commands::stream::submit_playurl_response,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RAM-Stream-Browser");
}
