//! mpv player launcher module.
//! Sprint 2: Spawns mpv.exe as a child process with DASH streams and 2GB RAM buffer.
//! Future: Replace subprocess with libmpv native bindings for tighter integration.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use crate::parsers::CookieEntry;

/// Global handle to the running mpv process, so we can kill it from other threads.
static MPV_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

/// Locate mpv.exe on the system.
/// Checks PATH first (via `mpv --version`), then common Windows install locations.
fn find_mpv() -> Option<PathBuf> {
    log::info!("[mpv] Searching for mpv.exe...");

    // 1. Check PATH — try running `mpv --version`
    log::info!("[mpv] Checking PATH: running 'mpv --version'...");
    match Command::new("mpv")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            log::info!("[mpv] Found on PATH (mpv --version succeeded)");
            return Some(PathBuf::from("mpv"));
        }
        Ok(status) => {
            log::info!("[mpv] 'mpv --version' exited with code {:?} — not on PATH", status.code());
        }
        Err(e) => {
            log::info!("[mpv] 'mpv --version' failed to run: {} — checking install locations...", e);
        }
    }

    // 2. Check common Windows install locations
    let candidates = [
        r"C:\Program Files\MPV Player\mpv.exe",
        r"C:\Program Files\mpv\mpv.exe",
        r"C:\Program Files (x86)\mpv\mpv.exe",
        r"C:\mpv\mpv.exe",
    ];

    for candidate in &candidates {
        let exists = std::path::Path::new(candidate).exists();
        log::info!("[mpv] Check {} → {}", candidate, if exists { "FOUND" } else { "not found" });
        if exists {
            log::info!("[mpv] Found at: {}", candidate);
            return Some(PathBuf::from(candidate));
        }
    }

    log::error!("[mpv] mpv.exe not found! Checked PATH and: {:?}", candidates);
    None
}

/// Launch mpv.exe in its own window with the given DASH streams and 2GB RAM buffer.
///
/// `playinfo_json` — raw __playinfo__ JSON string from B站
/// `cookies` — parsed cookie entries for CDN authentication
/// `quality_id` — specific quality ID to use (None = auto-select highest)
///
/// This function blocks the calling thread until mpv exits.
/// Call it from a spawned thread.
pub fn launch_player(
    playinfo_json: &str,
    cookies: &[CookieEntry],
    quality_id: Option<u32>,
) -> Result<(), String> {
    log::info!("[mpv] launch_player() entered");
    // Parse the playinfo JSON to extract DASH URLs
    let value: serde_json::Value = serde_json::from_str(playinfo_json)
        .map_err(|e| format!("Failed to parse playinfo JSON: {}", e))?;

    let (video_url, audio_url) = crate::parsers::extract_dash_urls(&value, quality_id)
        .ok_or("No DASH stream URLs found in playinfo — video may not be available")?;
    log::info!("[mpv] DASH URLs extracted: video={}..., audio={}",
        &video_url[..video_url.len().min(50)],
        audio_url.as_ref().map(|a| &a[..a.len().min(50)]).unwrap_or("none"));

    log::info!("[mpv] Video URL: {}", &video_url[..video_url.len().min(100)]);
    if let Some(ref audio) = audio_url {
        log::info!("[mpv] Audio URL: {}", &audio[..audio.len().min(100)]);
    }

    // Build cookie header string
    let cookie_str = cookies
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ");

    log::info!("[mpv] Cookie header length: {} chars", cookie_str.len());

    // User-Agent must match the WebView2's UA for B站 CDN to accept requests
    let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                      (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";

    // Build mpv command
    let mpv_path = find_mpv().ok_or("mpv.exe not found on this system. Install mpv or place mpv.exe in PATH.")?;
    log::info!("[mpv] mpv found at: {}", mpv_path.display());
    let mut cmd = Command::new(&mpv_path);
    cmd.arg(&video_url)
        // ── RAM buffer configuration (2GB forward, 512MB backward) ──
        .arg("--cache=yes")
        .arg("--demuxer-max-bytes=2147483648")       // 2 GiB forward readahead
        .arg("--demuxer-max-back-bytes=536870912")   // 512 MiB backward for instant rewind
        .arg("--cache-secs=3600")                    // Cap at 1 hour of content
        // ── Cookie & anti-hotlink headers ──
        .arg(format!("--http-header-fields=Cookie: {}", cookie_str))
        .arg("--http-header-fields=Referer: https://www.bilibili.com")
        .arg("--http-header-fields=Origin: https://www.bilibili.com")
        .arg(format!("--user-agent={}", user_agent))
        // ── Window & playback settings ──
        .arg("--no-ytdl")                            // Disable yt-dlp — we have direct URLs
        .arg("--keep-open=always")                   // Stay open even on error
        .arg("--force-window=yes")                   // Force window creation
        .arg("--title=RAM-Stream Player")
        .arg("--volume=80")
        .arg("--autofit=75%");                       // 75% of screen size

    // Attach audio track if separate (DASH)
    if let Some(ref audio) = audio_url {
        cmd.arg(format!("--audio-file={}", audio));
    }

    log::info!("[mpv] Spawning: mpv {}", cmd.get_args()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" "));

    // Capture stderr for diagnostics
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Spawn the process
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn mpv.exe: {}\nIs mpv installed and on PATH?", e))?;

    let pid = child.id();
    log::info!("[mpv] mpv.exe spawned OK, PID={}", pid);

    // Read stderr in a separate thread so it doesn't block
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    eprintln!("[mpv-stderr] {}", line);
                }
            }
        });
    }

    // Read stdout too
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    eprintln!("[mpv-stdout] {}", line);
                }
            }
        });
    }

    // Store the child handle so stop_player() can kill it
    if let Ok(mut proc) = MPV_PROCESS.lock() {
        *proc = Some(child);
    }

    // Wait for mpv to exit (blocks this thread)
    let mut child = MPV_PROCESS.lock()
        .map_err(|e| format!("Lock error: {}", e))?
        .take()
        .ok_or("Process handle disappeared")?;

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for mpv: {}", e))?;

    log::info!("[mpv] Process exited with status: {:?}", status);

    Ok(())
}

/// Kill the currently running mpv process (if any).
/// Called from the protocol handler when user clicks "Stop Player" or navigates away.
pub fn stop_player() {
    if let Ok(mut proc) = MPV_PROCESS.lock() {
        if let Some(mut child) = proc.take() {
            log::info!("[mpv] Stopping player (PID={})", child.id());
            let _ = child.kill();
            let _ = child.wait();
            log::info!("[mpv] Player stopped");
        } else {
            log::info!("[mpv] No player running to stop");
        }
    }
}

/// Check if mpv.exe is available on the system.
pub fn is_mpv_available() -> bool {
    find_mpv().is_some()
}
