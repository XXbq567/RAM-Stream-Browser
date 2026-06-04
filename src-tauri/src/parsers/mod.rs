/// Shared parsing module for RAM-Stream-Browser.
/// Consolidated types and functions for cookie parsing, playinfo analysis,
/// and DASH URL extraction — single source of truth across commands and protocol handlers.

use serde::{Deserialize, Serialize};

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayinfoSummary {
    pub has_dash: bool,
    pub video_qualities: Vec<QualityInfo>,
    pub audio_qualities: Vec<QualityInfo>,
    pub quality_descriptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityInfo {
    pub id: u32,
    pub codec: String,
    pub bandwidth: u64,
    pub quality_desc: String,
}

// ── Cookie parsing ─────────────────────────────────────────────────────────

/// Simple parser for document.cookie string.
/// Format: "name1=value1; name2=value2"
pub fn parse_cookie_string(raw: &str) -> Vec<CookieEntry> {
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

// ── Playinfo parsing ───────────────────────────────────────────────────────

/// Summarize parsed playinfo JSON into a PlayinfoSummary.
pub fn summarize_playinfo(value: &serde_json::Value) -> PlayinfoSummary {
    let dash = value.get("data").and_then(|d| d.get("dash"));

    let mut video_qualities = Vec::new();
    let mut audio_qualities = Vec::new();
    let mut quality_descriptions = Vec::new();

    if let Some(dash) = dash {
        // Video tracks
        if let Some(videos) = dash.get("video").and_then(|v| v.as_array()) {
            for v in videos {
                let id = v.get("id").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
                let codec_id = v.get("codecid").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                let bandwidth = v.get("bandwidth").and_then(|b| b.as_u64()).unwrap_or(0);
                let desc = format!(
                    "{} {} ({}bps)",
                    quality_id_to_desc(id),
                    codec_id_to_str(codec_id),
                    format_bandwidth(bandwidth)
                );
                quality_descriptions.push(desc.clone());
                video_qualities.push(QualityInfo {
                    id,
                    codec: codec_id_to_str(codec_id).to_string(),
                    bandwidth,
                    quality_desc: desc,
                });
            }
        }

        // Audio tracks
        if let Some(audios) = dash.get("audio").and_then(|a| a.as_array()) {
            for a in audios {
                let id = a.get("id").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
                let bandwidth = a.get("bandwidth").and_then(|b| b.as_u64()).unwrap_or(0);
                audio_qualities.push(QualityInfo {
                    id,
                    codec: "audio".to_string(),
                    bandwidth,
                    quality_desc: format!("Audio q{}", id),
                });
            }
        }
    }

    PlayinfoSummary {
        has_dash: dash.is_some(),
        video_qualities,
        audio_qualities,
        quality_descriptions,
    }
}

/// Parse a JSON string and summarize it.
pub fn summarize_playinfo_str(json_str: &str) -> Result<PlayinfoSummary, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;
    Ok(summarize_playinfo(&value))
}

// ── Quality / codec helpers ────────────────────────────────────────────────

/// B站 quality ID to human-readable description.
pub fn quality_id_to_desc(id: u32) -> &'static str {
    match id {
        127 => "8K",
        125 => "4K HDR",
        120 => "4K",
        116 => "1080p60 HDR",
        112 => "1080p60+",
        80 => "1080p",
        74 => "720p60",
        64 => "720p",
        48 => "720p (low)",
        32 => "480p",
        16 => "360p",
        _ => "unknown",
    }
}

/// B站 codec ID to human-readable name.
pub fn codec_id_to_str(id: u32) -> &'static str {
    match id {
        7 => "AVC/H.264",
        12 => "HEVC/H.265",
        13 => "AV1",
        _ => "unknown",
    }
}

/// Format bandwidth in human-readable form (e.g. "15.5M", "800K").
pub fn format_bandwidth(bps: u64) -> String {
    if bps >= 1_000_000 {
        format!("{:.1}M", bps as f64 / 1_000_000.0)
    } else if bps >= 1_000 {
        format!("{:.0}K", bps as f64 / 1_000.0)
    } else {
        format!("{}", bps)
    }
}

// ── DASH URL extraction ────────────────────────────────────────────────────

/// Try to read a field from a JSON object, checking both snake_case and camelCase names.
pub fn get_field<'a>(obj: &'a serde_json::Value, snake: &str, camel: &str) -> Option<&'a str> {
    obj.get(snake)
        .or_else(|| obj.get(camel))
        .and_then(|v| v.as_str())
}

/// Extract DASH video/audio base URLs from the playinfo JSON structure.
///
/// Navigates `data.dash.video[]` and `data.dash.audio[]`.
/// - If `quality_id` is provided, finds the matching video stream by its `id` field.
/// - If `quality_id` is `None`, picks the highest-bandwidth video stream.
/// - Always picks the highest-bandwidth audio stream (optional).
/// - Handles both `base_url`/`baseUrl` and falls back to `backup_url`/`backupUrl`.
///
/// Returns `(video_url, optional_audio_url)` or `None` if no DASH data is found.
pub fn extract_dash_urls(
    value: &serde_json::Value,
    quality_id: Option<u32>,
) -> Option<(String, Option<String>)> {
    let dash = value.get("data").and_then(|d| d.get("dash"))?;

    let videos = dash.get("video").and_then(|v| v.as_array())?;

    // Pick video stream
    let video = if let Some(qid) = quality_id {
        videos
            .iter()
            .find(|v| v.get("id").and_then(|i| i.as_u64()).map(|i| i as u32) == Some(qid))?
    } else {
        videos
            .iter()
            .max_by_key(|v| v.get("bandwidth").and_then(|b| b.as_u64()).unwrap_or(0))?
    };

    // Pick audio stream (always highest bandwidth), but audio is optional
    let audios = dash.get("audio").and_then(|a| a.as_array());
    let audio_count = audios.map(|a| a.len()).unwrap_or(0);
    log::info!("[mpv] Audio tracks available: {}", audio_count);

    let audio = audios.and_then(|audios| {
        audios
            .iter()
            .max_by_key(|a| a.get("bandwidth").and_then(|b| b.as_u64()).unwrap_or(0))
    });

    // Extract video URL: try base_url/baseUrl, fall back to backup_url/backupUrl
    let video_url = get_field(video, "base_url", "baseUrl")
        .or_else(|| get_field(video, "backup_url", "backupUrl"))?
        .to_string();

    // Extract audio URL (optional): same fallback strategy
    let audio_url = audio
        .and_then(|a| {
            get_field(a, "base_url", "baseUrl")
                .or_else(|| get_field(a, "backup_url", "backupUrl"))
        })
        .map(|s| s.to_string());

    Some((video_url, audio_url))
}
