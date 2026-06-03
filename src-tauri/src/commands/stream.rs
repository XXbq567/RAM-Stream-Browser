/// Stream interception commands.
/// Sprint 1: Receive and log __playinfo__ and playurl responses from the frontend.
/// Sprint 2: Parse DASH streams, select best quality, feed to libmpv.

use serde::{Deserialize, Serialize};

/// Receive the raw __playinfo__ JSON string from the frontend (Method B: DOM injection).
#[tauri::command]
pub fn submit_playinfo(json_str: String) -> Result<PlayinfoSummary, String> {
    let value: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let summary = summarize_playinfo(&value);
    log::info!("[PlayInfo] {:?}", summary);
    Ok(summary)
}

/// Receive intercepted playurl API response (Method A: network layer).
#[tauri::command]
pub fn submit_playurl_response(url: String, body: String) -> Result<PlayinfoSummary, String> {
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let summary = summarize_playinfo(&value);
    log::info!("[PlayURL Intercepted] url={} | {:?}", url, summary);
    Ok(summary)
}

/// Summarize the playinfo/playurl response for Sprint 1 logging.
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

/// B站 quality ID to human-readable description
fn quality_id_to_desc(id: u32) -> &'static str {
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

/// B站 codec ID to string
fn codec_id_to_str(id: u32) -> &'static str {
    match id {
        7 => "AVC/H.264",
        12 => "HEVC/H.265",
        13 => "AV1",
        _ => "unknown",
    }
}

fn summarize_playinfo(value: &serde_json::Value) -> PlayinfoSummary {
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

    let summary = PlayinfoSummary {
        has_dash: dash.is_some(),
        video_qualities,
        audio_qualities,
        quality_descriptions,
    };
    summary
}

fn format_bandwidth(bps: u64) -> String {
    if bps >= 1_000_000 {
        format!("{:.1}M", bps as f64 / 1_000_000.0)
    } else if bps >= 1_000 {
        format!("{:.0}K", bps as f64 / 1_000.0)
    } else {
        format!("{}", bps)
    }
}
