//! Generic resilient model file downloader.
//!
//! Why we don't lean on `hf-hub` (used internally by fastembed): in
//! production it has hit `request error: timeout: global` on slow links
//! because ureq's defaults aren't tuned for large files over flaky
//! connections, and the timeout isn't user-configurable through
//! `fastembed::InitOptions`. Owning the download ourselves also gives us
//! progress events we can stream to the UI.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::engine::events;

/// Per-request timeout — generous for slow networks but not unbounded.
const DOWNLOAD_TIMEOUT_SECS: u64 = 30 * 60;
/// Hard cap on a single file. Guards against a hijacked endpoint streaming
/// forever.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// How often to emit a `downloading` event during a download.
const PROGRESS_TICK_MS: u128 = 200;

/// Skip download when the file is already present locally.
pub fn ensure_file(
    path: &Path,
    url: &str,
    event_id: &str,
    label: &str,
) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create dir {parent:?}: {error}"))?;
    }
    tracing::info!("downloading {} -> {}", url, path.display());
    events::emit_starting(event_id, label);

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .build();

    let response = agent.get(url).call().map_err(|error| {
        let msg = format!("download failed ({url}): {error}");
        events::emit_error(event_id, label, &msg);
        msg
    })?;

    let total: Option<u64> = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());

    let mut reader = response.into_reader().take(MAX_FILE_BYTES);
    let mut bytes: Vec<u8> = Vec::with_capacity(total.unwrap_or(8 * 1024 * 1024) as usize);
    let mut buf = [0u8; 64 * 1024];
    let mut last_emit = Instant::now();

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&buf[..n]);
                if last_emit.elapsed().as_millis() >= PROGRESS_TICK_MS {
                    events::emit_downloading(event_id, label, bytes.len() as u64, total);
                    last_emit = Instant::now();
                }
            }
            Err(error) => {
                let msg = format!("download read failed ({url}): {error}");
                events::emit_error(event_id, label, &msg);
                return Err(msg);
            }
        }
    }

    if bytes.len() as u64 >= MAX_FILE_BYTES {
        let msg = format!("download {url} exceeded {MAX_FILE_BYTES} bytes; aborting");
        events::emit_error(event_id, label, &msg);
        return Err(msg);
    }

    // Final tick so the bar tops out before `ready`.
    events::emit_downloading(event_id, label, bytes.len() as u64, total);

    // Atomic install: write to a sibling temp path then rename so a partial
    // download can't leave a corrupted file in place.
    let tmp_path: PathBuf = path.with_extension(format!(
        "{}.partial",
        path.extension().and_then(|s| s.to_str()).unwrap_or("tmp")
    ));
    std::fs::write(&tmp_path, &bytes)
        .map_err(|error| format!("failed to write temp file: {error}"))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|error| format!("failed to install file: {error}"))?;

    tracing::info!("downloaded {} ({} bytes)", path.display(), bytes.len());
    events::emit_ready(event_id, label);
    Ok(())
}

/// Download a single file silently (no progress events). Useful for small
/// companion files where the UX cost of a banner outweighs the benefit.
pub fn ensure_file_quiet(path: &Path, url: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create dir {parent:?}: {error}"))?;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .build();
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("download failed ({url}): {error}"))?;
    let mut reader = response.into_reader().take(MAX_FILE_BYTES);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("download read failed ({url}): {error}"))?;
    let tmp_path: PathBuf = path.with_extension(format!(
        "{}.partial",
        path.extension().and_then(|s| s.to_str()).unwrap_or("tmp")
    ));
    std::fs::write(&tmp_path, &bytes)
        .map_err(|error| format!("failed to write temp file: {error}"))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|error| format!("failed to install file: {error}"))?;
    Ok(())
}
