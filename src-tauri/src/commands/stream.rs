/// Stream interception commands.
/// Sprint 1: Receive and log __playinfo__ and playurl responses from the frontend.
/// Sprint 2: Store in AppState and trigger mpv player launch.

use crate::parsers::*;
use crate::state::global_state;
use crate::protocol;

/// Receive the raw __playinfo__ JSON string from the frontend (Method B: DOM injection).
/// Store in AppState and trigger player launch if cookies are also available.
#[tauri::command]
pub fn submit_playinfo(json_str: String) -> Result<PlayinfoSummary, String> {
    let value: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let summary = summarize_playinfo(&value);
    log::info!("[PlayInfo] {:?}", summary);

    // Store in AppState for player launch
    if let Ok(mut s) = global_state().lock() {
        s.playinfo_json = Some(json_str.clone());
        s.playinfo_summary = Some(summary.clone());
        // Extract video_id from the playinfo JSON for deduplication
        s.current_video_id = value.get("data").and_then(|d| d.get("aid"))
            .and_then(|a| a.as_u64())
            .map(|id| id.to_string())
            .or_else(|| {
                value.get("data").and_then(|d| d.get("bvid"))
                    .and_then(|b| b.as_str())
                    .map(|s| s.to_string())
            });
        log::info!("[PlayInfo] Stored in AppState (via IPC), video_id={:?}", s.current_video_id);
    }

    // Trigger player launch if cookies are also available
    protocol::try_launch_player();

    Ok(summary)
}

/// Receive intercepted playurl API response (Method A: network layer).
/// Store in AppState and trigger player launch if cookies are also available.
#[tauri::command]
pub fn submit_playurl_response(url: String, body: String) -> Result<PlayinfoSummary, String> {
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let summary = summarize_playinfo(&value);
    log::info!("[PlayURL Intercepted] url={} | {:?}", url, summary);

    // Store in AppState for player launch (playurl response contains actual DASH URLs)
    if let Ok(mut s) = global_state().lock() {
        s.playinfo_json = Some(body.clone());
        s.playinfo_summary = Some(summary.clone());
        s.current_video_id = value.get("data").and_then(|d| d.get("aid"))
            .and_then(|a| a.as_u64())
            .map(|id| id.to_string())
            .or_else(|| {
                value.get("data").and_then(|d| d.get("bvid"))
                    .and_then(|b| b.as_str())
                    .map(|s| s.to_string())
            });
        log::info!("[PlayURL] Stored in AppState (via IPC), video_id={:?}", s.current_video_id);
    }

    // Trigger player launch if cookies are also available
    protocol::try_launch_player();

    Ok(summary)
}
