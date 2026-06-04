//! NOTE: The ram-stream:// custom protocol is currently non-functional.
//! WebView2 blocks all requests to unknown URL schemes from HTTPS origins.
//! The actual IPC path is: preload.js → bridge.html → Tauri IPC.
//! This module is kept for reference and potential future use if the
//! protocol registration issue is resolved.

/// Custom URI scheme protocol handler.
///
/// Since Tauri 2.0 does not inject __TAURI_INTERNALS__ on external domains
/// (e.g., bilibili.com), we use a custom `ram-stream://` protocol as the IPC bridge.
///
/// The preload script sends data via img beacon:
///   new Image().src = 'ram-stream://localhost/<command>?d=<url-encoded-json>'
///
/// This handler receives those requests, routes to the appropriate command logic,
/// and returns JSON responses.

use tauri::http::status::StatusCode;
use crate::parsers::*;
use crate::state::global_state;

/// 1x1 transparent PNG — returned for img beacon responses
const PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41,
    0x54, 0x78, 0x9C, 0x62, 0x00, 0x00, 0x00, 0x02,
    0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42,
    0x60, 0x82,
];

/// Extract the command name from the request URI.
/// URI format: ram-stream://localhost/submit-cookies
fn extract_command(uri: &str) -> &str {
    // Strip scheme
    let after_scheme = uri
        .strip_prefix("ram-stream://")
        .unwrap_or(uri);
    // Strip authority (localhost/)
    let path = after_scheme
        .strip_prefix("localhost/")
        .unwrap_or(after_scheme);
    // Strip query string
    path.split('?').next().unwrap_or(path)
}

fn extract_query_data(uri: &str) -> String {
    if let Some(query) = uri.split('?').nth(1) {
        for pair in query.split('&') {
            if let Some(val) = pair.strip_prefix("d=") {
                return url_decode(val);
            }
        }
    }
    String::new()
}

fn url_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                bytes.push(byte);
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            // Literal chars from encodeURIComponent are always ASCII-safe
            if (c as u32) < 128 {
                bytes.push(c as u8);
            }
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

/// Handle incoming ram-stream:// protocol requests.
pub fn handle(
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let uri = request.uri().to_string();
    let cmd = extract_command(&uri);
    let body_bytes = request.body().clone();
    let body_str = String::from_utf8_lossy(&body_bytes);

    log::info!("[Protocol] ===== REQUEST RECEIVED =====");
    log::info!("[Protocol] URI: {}", uri);
    log::info!("[Protocol] CMD: {}", cmd);
    log::info!("[Protocol] BODY bytes: {}", body_bytes.len());
    log::info!("[Protocol] BODY preview: {}", &body_str.chars().take(200).collect::<String>());

    // Read data from query string (img beacon) or request body (XHR fallback)
    let query_data = extract_query_data(&uri);
    if !query_data.is_empty() {
        log::info!("[Protocol] QUERY data: {} bytes", query_data.len());
    }
    let effective_data = if !query_data.is_empty() { query_data } else { body_str.to_string() };

    let (status, response_json) = match cmd {
        "submit_cookies" => {
            handle_submit_cookies(&effective_data)
        }
        "submit_playinfo" => {
            handle_submit_playinfo(&effective_data)
        }
        "submit_playurl_response" => {
            handle_submit_playurl_response(&effective_data)
        }
        "select_quality" => {
            log::info!("[Control] Quality selection requested");
            let quality_id: Option<u32> = if let Ok(val) = serde_json::from_str::<serde_json::Value>(&effective_data) {
                val.get("quality_id").and_then(|v| v.as_u64()).map(|i| i as u32)
            } else {
                None
            };

            if let Some(qid) = quality_id {
                if let Ok(mut s) = global_state().lock() {
                    s.selected_quality_id = Some(qid);
                }
                log::info!("[Control] Switching to quality id={}", qid);

                // If player is running, we'd need to restart it with new quality
                // For MVP: mark player_running=false so next playinfo triggers new launch
                if let Ok(mut s) = global_state().lock() {
                    if s.player_running {
                        s.player_running = false;
                        log::info!("[Control] Player will restart with new quality on next trigger");
                        // Re-trigger launch with new quality
                        drop(s);
                        try_launch_player();
                    }
                }
            }
            (StatusCode::OK, r#"{"ok":true,"action":"select_quality"}"#.to_string())
        }
        "stop_player" => {
            log::info!("[Control] Stop player requested");
            crate::mpv::stop_player();
            if let Ok(mut s) = global_state().lock() {
                s.player_running = false;
            }
            (StatusCode::OK, r#"{"ok":true,"action":"stop_player"}"#.to_string())
        }
        "ping" => {
            log::info!("[Protocol] 🟢 Ping received — protocol bridge is WORKING");
            (StatusCode::OK, r#"{"ok":true,"msg":"pong","bridge":"ram-stream"}"#.to_string())
        }
        _ => {
            log::warn!("[Protocol] Unknown command: {}", cmd);
            (
                StatusCode::NOT_FOUND,
                format!("{{\"error\":\"Unknown command: {}\"}}", cmd),
            )
        }
    };

    // Return transparent PNG for img beacon; JSON for ping/errors
    match cmd {
        "submit_cookies" | "submit_playinfo" | "submit_playurl_response" | "select_quality" | "stop_player" => {
            tauri::http::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "image/png")
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "no-store")
                .body(PIXEL_PNG.to_vec())
                .expect("valid PNG response")
        }
        "ping" => {
            tauri::http::Response::builder()
                .status(status)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(response_json.into_bytes())
                .expect("valid response")
        }
        _ => {
            tauri::http::Response::builder()
                .status(status)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(response_json.into_bytes())
                .expect("valid response")
        }
    }
}

/// Handle submit_cookies: parse document.cookie string.
fn handle_submit_cookies(body: &str) -> (StatusCode, String) {
    log::info!("[Cookie] Raw body: {}", &body.chars().take(500).collect::<String>());
    // Parse { "cookies": "..." } from the JSON body
    let cookies_str = if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        val.get("cookies")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        // Fallback: treat entire body as raw cookie string
        body.to_string()
    };

    let entries = parse_cookie_string(&cookies_str);

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

    let has_sessdata = entries.iter().any(|e| e.name == "SESSDATA");
    log::info!("[Cookie] Total cookies received: {}", entries.len());
    if !has_sessdata {
        log::warn!("[Cookie] ⚠️ SESSDATA not found! (HttpOnly — attempting COM CookieManager)");
    }

    // Store cookies in AppState for player launch
    let state = global_state();
    if let Ok(mut s) = state.lock() {
        s.cookies = Some(entries.clone());
        log::info!("[Cookie] Stored {} cookies in AppState", entries.len());
    }
    // Try to launch player if all conditions are met
    try_launch_player();

    let json = serde_json::json!({
        "ok": true,
        "count": entries.len(),
        "entries": entries
    });
    (StatusCode::OK, json.to_string())
}

/// Handle submit_playinfo: parse __playinfo__ JSON.
fn handle_submit_playinfo(body: &str) -> (StatusCode, String) {
    log::info!("[PlayInfo] Raw body preview: {}", &body.chars().take(500).collect::<String>());
    let json_str = if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        val.get("jsonStr")
            .and_then(|v| v.as_str())
            .unwrap_or(body)
            .to_string()
    } else {
        body.to_string()
    };

    let summary = summarize_playinfo_str(&json_str);
    match &summary {
        Ok(s) => log::info!("[PlayInfo] {:?}", s),
        Err(e) => log::error!("[PlayInfo] Parse error: {}", e),
    }

    // Store parsed playinfo in AppState for player launch
    let state = global_state();
    if let Ok(mut s) = state.lock() {
        s.playinfo_json = Some(json_str.clone());
        if let Ok(ref summary) = summary {
            s.playinfo_summary = Some(summary.clone());
            // Extract video_id from the playinfo JSON for deduplication
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                s.current_video_id = val.get("data").and_then(|d| d.get("aid"))
                    .and_then(|a| a.as_u64())
                    .map(|id| id.to_string())
                    .or_else(|| {
                        val.get("data").and_then(|d| d.get("bvid"))
                            .and_then(|b| b.as_str())
                            .map(|s| s.to_string())
                    });
            }
            log::info!("[PlayInfo] Stored in AppState, video_id={:?}", s.current_video_id);
        }
    }
    // Try to launch player if all conditions are met
    try_launch_player();

    let response = match summary {
        Ok(s) => serde_json::json!({ "ok": true, "summary": s }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    (StatusCode::OK, response.to_string())
}

/// Handle submit_playurl_response: intercepted playurl API response.
fn handle_submit_playurl_response(body: &str) -> (StatusCode, String) {
    let (url, response_body) = if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        let u = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let b = val.get("body").and_then(|v| v.as_str()).unwrap_or(body).to_string();
        (u, b)
    } else {
        (String::new(), body.to_string())
    };

    let summary = summarize_playinfo_str(&response_body);
    match &summary {
        Ok(s) => log::info!("[PlayURL Intercepted] url={} | {:?}", url, s),
        Err(e) => log::error!("[PlayURL] Parse error: {}", e),
    }

    // Store in AppState (playurl response contains actual DASH URLs with tokens)
    let state = global_state();
    if let Ok(mut s) = state.lock() {
        s.playinfo_json = Some(response_body.clone());
        if let Ok(ref summary) = summary {
            s.playinfo_summary = Some(summary.clone());
            // Extract video_id from the response JSON for deduplication
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&response_body) {
                s.current_video_id = val.get("data").and_then(|d| d.get("aid"))
                    .and_then(|a| a.as_u64())
                    .map(|id| id.to_string())
                    .or_else(|| {
                        val.get("data").and_then(|d| d.get("bvid"))
                            .and_then(|b| b.as_str())
                            .map(|s| s.to_string())
                    });
            }
            log::info!("[PlayURL] Stored in AppState, video_id={:?}", s.current_video_id);
        }
    }
    // Try to launch player if all conditions are met
    try_launch_player();

    let response = match summary {
        Ok(s) => serde_json::json!({ "ok": true, "summary": s }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    (StatusCode::OK, response.to_string())
}

/// Check preconditions and launch mpv player if ready.
/// Requires: cookies present, playinfo summary available, player not already running.
/// Called from protocol handler AND from Tauri IPC commands.
///
/// NOTE: COM cookie extraction (for HttpOnly cookies like SESSDATA) is triggered
/// separately when document.cookie is first submitted. By the time this function runs,
/// cookies should already include SESSDATA if the COM extraction succeeded.
pub fn try_launch_player() {
    let state = global_state();
    log::info!("[Launch] try_launch_player() called");

    // Lock once and check each condition individually with logging
    {
        let s = state.lock().unwrap();

        // Check 1: cookies
        if s.cookies.is_none() {
            log::warn!("[Launch] Not ready: no cookies");
            return;
        }
        // Check 2: playinfo summary
        if s.playinfo_summary.is_none() {
            log::warn!("[Launch] Not ready: no playinfo summary (playinfo_json={})",
                s.playinfo_json.is_some());
            return;
        }
        // Check 3: player already running
        if s.player_running {
            log::warn!("[Launch] Already running, skip (player_running=true)");
            return;
        }

        let has_sessdata = s.cookies.as_ref()
            .map(|c| c.iter().any(|e| e.name == "SESSDATA"))
            .unwrap_or(false);

        log::info!("[Launch] Conditions met — cookies:{} entries, playinfo:{} qualities, SESSDATA:{}",
            s.cookies.as_ref().map(|c| c.len()).unwrap_or(0),
            s.playinfo_summary.as_ref().map(|p| p.video_qualities.len()).unwrap_or(0),
            if has_sessdata { "yes" } else { "no" }
        );

        if !has_sessdata {
            log::warn!(
                "[Launch] ⚠️ SESSDATA still missing! CDN may return 403. \
                COM extraction may still be in progress."
            );
        }
    }

    // Set player_running BEFORE spawning thread to prevent duplicate launches
    let (cookies, playinfo_json, quality_id) = {
        let mut s = state.lock().unwrap();
        s.player_running = true;
        let qid = s.selected_quality_id;
        (s.cookies.clone().unwrap(), s.playinfo_json.clone().unwrap(), qid)
    };

    log::info!("[Launch] Starting mpv player (quality={:?}, {} cookies)...",
        quality_id, cookies.len());

    let state_clone = global_state().clone();
    std::thread::spawn(move || {
        log::info!("[Launch] mpv launch thread started");
        match crate::mpv::launch_player(&playinfo_json, &cookies, quality_id) {
            Ok(()) => {
                log::info!("[Launch] Player exited normally");
            }
            Err(e) => {
                log::error!("[Launch] Player error: {}", e);
            }
        }
        // Mark player as stopped
        if let Ok(mut s) = state_clone.lock() {
            s.player_running = false;
            log::info!("[Launch] Player flag reset");
        }
    });
}
