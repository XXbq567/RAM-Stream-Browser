/// RAM-Stream-Browser: Crate root.
/// Sprint 1: Tauri shell + Webview2 → loads B站, extracts cookies & __playinfo__.

pub mod adapters;
pub mod commands;
pub mod error;
pub mod ipc;
pub mod mpv;
pub mod parsers;
pub mod protocol;
pub mod rules;
pub mod state;
pub mod updater;

use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;

use crate::state::{AppState, SharedState};

/// Preload script injected into every page (including B站 after navigation).
/// Runs before page scripts, stays active across SPA navigation.
const PRELOAD_SCRIPT: &str = include_str!("../../src/inject/preload.js");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        // Custom protocol bridge — since Tauri 2.0 does NOT inject __TAURI_INTERNALS__
        // on external domains (bilibili.com), the preload script communicates via
        // fetch('ram-stream://localhost/<command>', { method: 'POST', body: ... })
        .register_uri_scheme_protocol("ram-stream", move |_ctx, request| protocol::handle(request))
        .setup(|app| {
            // ── Initialize shared application state ──
            let shared_state: SharedState = Arc::new(Mutex::new(AppState::new()));
            state::init_global_state(shared_state.clone());
            app.manage(shared_state.clone());
            log::info!("[State] AppState initialized and managed");

            // Check if mpv is available
            if crate::mpv::is_mpv_available() {
                log::info!("[State] mpv.exe detected — player ready");
            } else {
                log::warn!("[State] mpv.exe NOT found — player will fail to launch");
            }

            // Use WebviewUrl::External so the WebView is in "internet" mode from the start.
            // This allows full navigation on B站 (clicks, SPA routing, etc.).
            // WebviewUrl::App would restrict navigation to the local origin.
            let _window = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(
                    tauri::Url::parse("https://www.bilibili.com")
                        .expect("valid B站 URL"),
                ),
            )
            .title("RAM-Stream-Browser")
            .inner_size(1600.0, 900.0)
            .resizable(true)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0")
            .initialization_script(PRELOAD_SCRIPT)
            // Navigation scope: allow only *.bilibili.com domains, block external links
            .on_navigation(|url| {
                let allowed = url.host_str()
                    .map(|h| h.ends_with("bilibili.com") || h == "localhost")
                    .unwrap_or(false);
                if allowed {
                    log::debug!("[Nav] ✅ Allowed: {}", url);
                } else {
                    log::warn!("[Nav] 🚫 Blocked external: {}", url);
                }
                allowed
            })
            .devtools(true)
            .build()?;

            log::info!(
                "[RAM-Stream] v{} — Window created, preload {} bytes, nav handlers enabled",
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
