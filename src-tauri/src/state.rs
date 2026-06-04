use std::sync::{Arc, Mutex};
use crate::parsers::{CookieEntry, PlayinfoSummary};

/// Central application state shared across all modules.
/// Managed by Tauri and also accessible via a global static for protocol handlers.
pub struct AppState {
    /// Cookies extracted from the B站 WebView (via document.cookie — no HttpOnly)
    pub cookies: Option<Vec<CookieEntry>>,
    /// Raw playinfo JSON string (for re-parsing with different quality)
    pub playinfo_json: Option<String>,
    /// Parsed playinfo summary with quality options
    pub playinfo_summary: Option<PlayinfoSummary>,
    /// Current B站 video ID (aid or bvid), used to detect navigation to new video
    pub current_video_id: Option<String>,
    /// Whether the mpv player is currently running
    pub player_running: bool,
    /// User-selected quality ID (None = auto-select highest)
    pub selected_quality_id: Option<u32>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cookies: None,
            playinfo_json: None,
            playinfo_summary: None,
            current_video_id: None,
            player_running: false,
            selected_quality_id: None,
        }
    }

    /// Clear playinfo-related state (when navigating away from video)
    pub fn clear_playinfo(&mut self) {
        self.playinfo_json = None;
        self.playinfo_summary = None;
        self.current_video_id = None;
    }

    /// Reset all state (on app restart or manual reset)
    pub fn reset(&mut self) {
        self.cookies = None;
        self.clear_playinfo();
        self.selected_quality_id = None;
        // player_running is managed by the mpv module
    }
}

/// Thread-safe shared state type used throughout the application.
pub type SharedState = Arc<Mutex<AppState>>;

// ── Global static access ──────────────────────────────────────────
// The protocol handler (register_uri_scheme_protocol) runs inside a
// Tauri callback that does not have direct access to AppHandle.
// A OnceLock provides access to the state without passing AppHandle.

static GLOBAL_STATE: std::sync::OnceLock<SharedState> = std::sync::OnceLock::new();

/// Initialize the global state. Must be called once during app setup,
/// before the protocol handler receives any requests.
pub fn init_global_state(state: SharedState) {
    GLOBAL_STATE
        .set(state)
        .map_err(|_| "Global state already initialized")
        .expect("AppState::init_global_state called more than once");
}

/// Get a reference to the global shared state.
/// Panics if init_global_state has not been called yet.
pub fn global_state() -> &'static SharedState {
    GLOBAL_STATE
        .get()
        .expect("AppState not initialized — call init_global_state first")
}
