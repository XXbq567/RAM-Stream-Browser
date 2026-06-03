/// Cookie extraction command.
/// Sprint 1: Receives cookies from the frontend (via document.cookie).
/// Sprint 2: Will also support direct Webview2 CookieManager COM access.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
    pub domain: String,
}

/// Simple parser for document.cookie string.
/// Format: "name1=value1; name2=value2"
fn parse_cookie_string(raw: &str) -> Vec<CookieEntry> {
    raw.split(';')
        .filter_map(|pair| {
            let pair = pair.trim();
            if pair.is_empty() {
                return None;
            }
            let mut parts = pair.splitn(2, '=');
            let name = parts.next()?.trim().to_string();
            let value = parts.next().unwrap_or("").trim().to_string();
            Some(CookieEntry {
                name,
                value,
                domain: ".bilibili.com".to_string(),
            })
        })
        .collect()
}

/// Receive the raw document.cookie string from the frontend.
/// Print key cookie names for Sprint 1 verification.
#[tauri::command]
pub fn submit_cookies(cookies: String) -> Result<Vec<CookieEntry>, String> {
    let entries = parse_cookie_string(&cookies);

    // Log key cookie names (redacted values for security)
    let key_names = ["SESSDATA", "bili_jct", "buvid3", "buvid4", "dedeuserid", "DedeUserID"];
    for entry in &entries {
        if key_names.contains(&entry.name.as_str()) {
            log::info!(
                "[Cookie] {} = {}...{} (len={})",
                entry.name,
                entry.value.chars().take(6).collect::<String>(),
                entry.value.chars().rev().take(6).collect::<String>().chars().rev().collect::<String>(),
                entry.value.len()
            );
        }
    }

    log::info!("[Cookie] Total cookies received: {}", entries.len());

    // Check for critical cookie: SESSDATA (大会员 auth)
    let has_sessdata = entries.iter().any(|c| c.name == "SESSDATA");
    if !has_sessdata {
        log::warn!("[Cookie] ⚠️ SESSDATA not found! User may not be logged in, or cookie is HttpOnly.");
        log::warn!("[Cookie] HttpOnly cookies cannot be read via document.cookie. Will use CookieManager API in Sprint 2.");
    }

    Ok(entries)
}
