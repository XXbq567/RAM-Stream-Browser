/// Platform adapter trait system.
/// Each supported platform implements the PlatformAdapter trait.
/// Sprint 2+ will register adapters keyed by URL pattern.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Universal stream representation extracted from any platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub video_url: String,
    pub audio_url: Option<String>,
    pub quality: String,
    pub codec: String,
    pub format: StreamFormat,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamFormat {
    Dash,
    Hls,
    Mp4,
}

/// Platform adapter contract.
/// Each platform (B站, YouTube, etc.) implements this trait.
pub trait PlatformAdapter: Send + Sync {
    fn url_patterns(&self) -> &'static [&'static str];
    fn parse_playinfo_json(
        &self,
        json: &serde_json::Value,
    ) -> Result<Vec<StreamInfo>, AdapterError>;
    fn preload_script(&self) -> &'static str;
}

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("URL does not match adapter pattern")]
    NoMatch,
    #[error("Failed to parse response: {0}")]
    ParseFailure(String),
    #[error("No suitable quality stream found")]
    NoSuitableStream,
}

// Sprint 2: pub mod bilibili;
// Sprint 3+: pub mod youtube;
