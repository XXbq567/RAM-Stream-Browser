/// Cookie extraction command.
/// Sprint 1: Receives cookies from the frontend (via document.cookie).
/// Sprint 2: Stores in AppState for mpv player launch.

use crate::parsers::{CookieEntry, parse_cookie_string};
use crate::state::global_state;
use crate::protocol;

/// Receive the raw document.cookie string from the frontend.
/// Store in AppState and trigger player launch if playinfo is also available.
#[tauri::command]
pub fn submit_cookies(cookies: String) -> Result<Vec<CookieEntry>, String> {
    let entries = parse_cookie_string(&cookies);

    // Log key cookie names (redacted values for security)
    let key_names = ["SESSDATA", "bili_jct", "buvid3", "buvid4", "dedeuserid", "DedeUserID"];
    for entry in &entries {
        if key_names.contains(&entry.name.as_str()) {
            log::info!(
                "[Cookie] {} = {}... (len={})",
                entry.name,
                &entry.value.chars().take(6).collect::<String>(),
                entry.value.len()
            );
        }
    }

    log::info!("[Cookie] Total cookies received: {}", entries.len());

    // Check for critical cookie: SESSDATA (大会员 auth)
    let has_sessdata = entries.iter().any(|c| c.name == "SESSDATA");
    if !has_sessdata {
        log::warn!("[Cookie] ⚠️ SESSDATA not found! User may not be logged in, or cookie is HttpOnly.");
        log::warn!("[Cookie] HttpOnly cookies cannot be read via document.cookie. Will use CookieManager API in Sprint 3.");
    }

    // Store in AppState for player launch
    if let Ok(mut s) = global_state().lock() {
        s.cookies = Some(entries.clone());
        log::info!("[Cookie] Stored {} cookies in AppState (via IPC)", entries.len());
    }

    // Trigger player launch if playinfo is also available
    protocol::try_launch_player();

    Ok(entries)
}
