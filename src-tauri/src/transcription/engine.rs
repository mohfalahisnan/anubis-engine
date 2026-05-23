//! Audio + video transcription via whisper.cpp invoked as a subprocess.
//!
//! Why subprocess (not the `whisper-rs` Rust binding):
//! `whisper-rs-sys` pulls in `bindgen`, which requires libclang on the build
//! host. That's a heavy install for users on a clean Windows machine and is
//! not solvable through Cargo features. By shipping no native deps and
//! shelling out to a downloaded whisper.cpp binary, we keep the build matrix
//! flat and let the same `ensure_file` helper that handles every other model
//! also handle the transcription artefacts.
//!
//! Pipeline:
//!   1. ffmpeg decodes the source file into 16 kHz mono s16le PCM WAV. We
//!      shell out to a raw `Command` (not `ffmpeg-sidecar`) so we can pipe
//!      stderr and surface the real failure reason on bad inputs.
//!   2. whisper.cpp CLI transcribes the WAV. Output is parsed from stdout —
//!      each segment line looks like `[hh:mm:ss.SSS --> hh:mm:ss.SSS]  text`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::engine::download::ensure_file;
use crate::EngineError;

/// Pinned to a release that's known to ship `whisper-bin-x64.zip`.
const WHISPER_BIN_VERSION: &str = "v1.7.6";
const WHISPER_BIN_URL_WIN64: &str =
    "https://github.com/ggerganov/whisper.cpp/releases/download/v1.7.6/whisper-bin-x64.zip";

/// Default Whisper model. Medium multilingual gives noticeably better
/// recall on non-English audio (Indonesian in particular) than the base
/// variant. Override with `ANUBIS_WHISPER_MODEL` — values match the suffix
/// of the upstream filename, e.g. `base`, `small`, `medium`, `large-v3`.
const DEFAULT_WHISPER_MODEL: &str = "medium";

/// Windows ffmpeg static build — BtbN's GPL build is widely used and ships
/// binaries that run on Windows 10+ without requiring extra runtime DLLs.
/// We pin a known-good asset on the floating `latest` release tag.
const FFMPEG_URL_WIN64: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

const WHISPER_DIR_NAME: &str = "transcription-whisper";
const FFMPEG_DIR_NAME: &str = "transcription-ffmpeg";
const FFMPEG_EVENT_ID: &str = "ffmpeg";
const FFMPEG_LABEL: &str = "ffmpeg (audio extractor ~80MB)";
const WHISPER_BIN_EVENT_ID: &str = "whisper-binary";
const WHISPER_BIN_LABEL: &str = "Whisper.cpp binary";
const WHISPER_MODEL_EVENT_ID: &str = "whisper-model";

static MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Cache only successful inits — a failure (e.g. transient network blip
/// during the first download) MUST NOT poison the cell, otherwise the user
/// has to restart the whole app to retry.
static ARTIFACTS: OnceLock<Artifacts> = OnceLock::new();
static INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn set_models_dir(path: PathBuf) {
    let _ = MODELS_DIR.set(path);
}

fn models_dir() -> PathBuf {
    if let Ok(env_dir) = std::env::var("ANUBIS_TRANSCRIPTION_MODELS_DIR") {
        return PathBuf::from(env_dir);
    }
    if let Some(dir) = MODELS_DIR.get() {
        return dir.clone();
    }
    std::env::temp_dir().join("anubis-transcription")
}

struct Artifacts {
    ffmpeg_exe: PathBuf,
    whisper_exe: PathBuf,
    whisper_model: PathBuf,
}

fn artifacts() -> Result<&'static Artifacts, EngineError> {
    if let Some(a) = ARTIFACTS.get() {
        return Ok(a);
    }
    // Serialise concurrent inits so two videos indexed back-to-back don't
    // both try to download the same files in parallel.
    let _guard = INIT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(a) = ARTIFACTS.get() {
        return Ok(a);
    }
    let built = init_artifacts().map_err(EngineError::Transcribe)?;
    Ok(ARTIFACTS.get_or_init(|| built))
}

fn init_artifacts() -> Result<Artifacts, String> {
    // ffmpeg — drive the download ourselves (ensure_file emits real progress
    // events) instead of any third-party auto_download which would be silent and
    // can hang the UI looking like nothing is happening. We honour a system
    // install if one is already on PATH.
    let ffmpeg_dir = models_dir().join(FFMPEG_DIR_NAME);
    std::fs::create_dir_all(&ffmpeg_dir).map_err(|error| format!("create ffmpeg dir: {error}"))?;
    let ffmpeg_exe = ensure_ffmpeg_binary(&ffmpeg_dir)?;

    // whisper.cpp binary (Windows only for now — see WHISPER_BIN_URL_WIN64).
    let whisper_dir = models_dir().join(WHISPER_DIR_NAME);
    std::fs::create_dir_all(&whisper_dir)
        .map_err(|error| format!("create whisper dir: {error}"))?;
    let whisper_exe = ensure_whisper_binary(&whisper_dir)?;

    let model_variant = whisper_model_variant();
    let (model_file, model_url, model_label) = whisper_model_artifact(&model_variant);
    let model_path = whisper_dir.join(&model_file);
    ensure_file(
        &model_path,
        &model_url,
        WHISPER_MODEL_EVENT_ID,
        &model_label,
    )?;

    Ok(Artifacts {
        ffmpeg_exe,
        whisper_exe,
        whisper_model: model_path,
    })
}

/// Resolve which Whisper model the user wants. Env var `ANUBIS_WHISPER_MODEL`
/// takes precedence; otherwise we fall back to [`DEFAULT_WHISPER_MODEL`].
fn whisper_model_variant() -> String {
    std::env::var("ANUBIS_WHISPER_MODEL")
        .ok()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_WHISPER_MODEL.to_string())
}

/// Map a model name (`base`, `small`, `medium`, `large-v3`, …) to its
/// filename, HuggingFace URL, and a banner label that includes the on-disk
/// size so the UI can show users what they're about to download.
fn whisper_model_artifact(variant: &str) -> (String, String, String) {
    let file = format!("ggml-{variant}.bin");
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}");
    let size_hint = match variant {
        "tiny" | "tiny.en" => "~75MB",
        "base" | "base.en" => "~145MB",
        "small" | "small.en" => "~480MB",
        "medium" | "medium.en" => "~1.5GB",
        "large-v1" | "large-v2" | "large-v3" | "large" => "~3GB",
        _ => "unknown size",
    };
    let label = format!("Whisper model ({variant} multilingual {size_hint})");
    (file, url, label)
}

#[cfg(target_os = "windows")]
fn ensure_ffmpeg_binary(dir: &Path) -> Result<PathBuf, String> {
    let exe = dir.join("ffmpeg.exe");

    // If we already have a binary, validate it before trusting it. A
    // previous run may have left a truncated or wrong-arch download behind
    // (Windows error 216, ERROR_EXE_MACHINE_TYPE_MISMATCH).
    if exe.exists() {
        if validate_ffmpeg(&exe).is_ok() {
            return Ok(exe);
        }
        tracing::warn!(
            "cached ffmpeg at {} is broken; re-downloading",
            exe.display()
        );
        let _ = std::fs::remove_file(&exe);
    }

    // Respect a system install — saves the download on dev machines.
    if let Ok(system_exe) = which_ffmpeg() {
        if validate_ffmpeg(&system_exe).is_ok() {
            return Ok(system_exe);
        }
    }

    let zip_path = dir.join("ffmpeg-win64.zip");
    // Belt-and-braces: a previous failed extract may have left this in place.
    let _ = std::fs::remove_file(&zip_path);
    ensure_file(&zip_path, FFMPEG_URL_WIN64, FFMPEG_EVENT_ID, FFMPEG_LABEL)?;

    // The release zip puts everything inside `<top>/bin/ffmpeg.exe`.
    // Match by suffix so any top-level directory name works.
    extract_specific_file(&zip_path, "bin/ffmpeg.exe", &exe)?;
    let _ = std::fs::remove_file(&zip_path);

    if !exe.exists() {
        return Err(format!(
            "ffmpeg.exe not found after extracting {}",
            zip_path.display()
        ));
    }

    // Final guard — if even the freshly-downloaded binary won't run, surface
    // a clear error instead of letting the indexer choke on every video.
    validate_ffmpeg(&exe).map_err(|error| {
        let _ = std::fs::remove_file(&exe);
        format!(
            "ffmpeg.exe downloaded but failed to execute ({error}). \
             The download may be corrupt or incompatible with this Windows build."
        )
    })?;

    Ok(exe)
}

/// Run `<exe> -version` and check the exit status. Anything other than a
/// clean zero-exit means we shouldn't trust this binary.
fn validate_ffmpeg(exe: &Path) -> Result<(), String> {
    let output = Command::new(exe)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("spawn {}: {error}", exe.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} -version exited with {:?}",
            exe.display(),
            output.status.code()
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn ensure_ffmpeg_binary(_dir: &Path) -> Result<PathBuf, String> {
    which_ffmpeg().map_err(|_| {
        "ffmpeg not found on PATH. Install it (e.g. `brew install ffmpeg` or your distro's \
         package manager) so transcription can decode audio."
            .to_string()
    })
}

fn which_ffmpeg() -> Result<PathBuf, String> {
    // Minimal PATH lookup — avoids pulling in the `which` crate for one call.
    let exe_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let path_var = std::env::var_os("PATH").ok_or_else(|| "PATH not set".to_string())?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!("{exe_name} not found on PATH"))
}

#[cfg(target_os = "windows")]
fn extract_specific_file(zip_path: &Path, suffix: &str, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|error| format!("open zip: {error}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| format!("read zip: {error}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("read entry {index}: {error}"))?;
        let Some(name) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        // Match anything ending in `bin/ffmpeg.exe` regardless of the
        // versioned top-level directory in the gyan.dev archives.
        if name.to_string_lossy().replace('\\', "/").ends_with(suffix) {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("create dir {parent:?}: {error}"))?;
            }
            let mut out_file = std::fs::File::create(dest)
                .map_err(|error| format!("create file {dest:?}: {error}"))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|error| format!("write file {dest:?}: {error}"))?;
            return Ok(());
        }
    }
    Err(format!(
        "no entry matching `{suffix}` in {}",
        zip_path.display()
    ))
}

#[cfg(target_os = "windows")]
fn ensure_whisper_binary(dir: &Path) -> Result<PathBuf, String> {
    // v1.7+ release zips ship BOTH `whisper-cli.exe` (the real binary) AND
    // `main.exe` (a deprecation shim that prints a warning and exits 1). We
    // must prefer `whisper-cli.exe` — picking `main.exe` would make every
    // transcription fail with a useless exit-1 deprecation message.
    //
    // The DLLs (whisper.dll, ggml.dll, …) must live in the same directory as
    // the binary at run time, hence we keep the whole extracted bundle.
    let marker = dir.join(format!(".whisper-{WHISPER_BIN_VERSION}"));
    let preferred = dir.join("whisper-cli.exe");
    let legacy = dir.join("main.exe");

    if marker.exists() {
        if preferred.exists() {
            return Ok(preferred);
        }
        if legacy.exists() {
            return Ok(legacy);
        }
        // Marker present but binary gone — fall through and re-extract.
    }

    let zip_path = dir.join("whisper-bin-x64.zip");
    let _ = std::fs::remove_file(&zip_path);
    ensure_file(
        &zip_path,
        WHISPER_BIN_URL_WIN64,
        WHISPER_BIN_EVENT_ID,
        WHISPER_BIN_LABEL,
    )?;

    extract_zip(&zip_path, dir)?;
    let _ = std::fs::remove_file(&zip_path);

    // Some release builds nest the contents inside `Release/`. Flatten if so.
    let release_subdir = dir.join("Release");
    if release_subdir.is_dir() {
        flatten_dir(&release_subdir, dir)?;
        let _ = std::fs::remove_dir_all(&release_subdir);
    }

    let chosen = if preferred.exists() {
        preferred
    } else if legacy.exists() {
        // Older release — only ships main.exe, no deprecation shim yet.
        legacy
    } else {
        return Err(format!(
            "whisper binary not found in {dir:?}; expected whisper-cli.exe or main.exe"
        ));
    };

    std::fs::write(&marker, WHISPER_BIN_VERSION)
        .map_err(|error| format!("write version marker: {error}"))?;
    Ok(chosen)
}

#[cfg(not(target_os = "windows"))]
fn ensure_whisper_binary(_dir: &Path) -> Result<PathBuf, String> {
    Err(
        "Whisper transcription is currently only auto-installed on Windows. \
         Install whisper.cpp manually and set ANUBIS_WHISPER_BIN to the binary path."
            .to_string(),
    )
}

#[cfg(target_os = "windows")]
fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file =
        std::fs::File::open(zip_path).map_err(|error| format!("open whisper zip: {error}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| format!("read whisper zip: {error}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("read entry {index}: {error}"))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out_path = dest.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|error| format!("create dir {out_path:?}: {error}"))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create dir {parent:?}: {error}"))?;
        }
        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|error| format!("create file {out_path:?}: {error}"))?;
        std::io::copy(&mut entry, &mut out_file)
            .map_err(|error| format!("write file {out_path:?}: {error}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn flatten_dir(src: &Path, dest: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(src).map_err(|error| format!("readdir {src:?}: {error}"))? {
        let entry = entry.map_err(|error| format!("dir entry: {error}"))?;
        let target = dest.join(entry.file_name());
        if target.exists() {
            let _ = std::fs::remove_file(&target);
        }
        std::fs::rename(entry.path(), &target)
            .map_err(|error| format!("move {:?}: {error}", entry.path()))?;
    }
    Ok(())
}

/// Decode any ffmpeg-supported audio/video container into a 16 kHz mono PCM
/// WAV file at `output_path`. Uses a raw `Command` instead of FfmpegCommand
/// so we can pipe stderr and surface the real reason ffmpeg refused a file
/// (otherwise users see a bare `status Some(-22)` with no context).
fn extract_audio_to_wav(
    ffmpeg_exe: &Path,
    input: &Path,
    output_path: &Path,
) -> Result<(), EngineError> {
    // `-loglevel error` keeps the stream small but still emits real failures.
    // `-y` overwrites any stale .wav from a previous attempt.
    // `-map 0:a:0` explicitly selects the first audio track — if the source
    // has no audio at all (common with screen-capture AVIs), ffmpeg fails
    // fast with `Stream specifier 0:a:0 matches no streams` instead of
    // creating an invalid zero-stream output file.
    let output = Command::new(ffmpeg_exe)
        .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(input)
        .args([
            "-map",
            "0:a:0",
            "-vn", // drop video stream (belt-and-braces with -map)
            "-sn", // drop subtitles
            "-ac",
            "1", // mono
            "-ar",
            "16000", // Whisper expects 16 kHz
            "-c:a",
            "pcm_s16le",
        ])
        .arg(output_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            EngineError::Transcribe(format!("spawn ffmpeg ({}): {error}", ffmpeg_exe.display()))
        })?;

    if !output.status.success() {
        let stderr_text = String::from_utf8_lossy(&output.stderr);
        // Clean up any half-written wav so it isn't mistaken for valid input
        // later. Best-effort — missing file is fine.
        let _ = std::fs::remove_file(output_path);

        let lower = stderr_text.to_ascii_lowercase();
        // Common shapes when the source has no audio stream:
        //   "Stream specifier '0:a:0' in filtergraph ... matches no streams."
        //   "Output file does not contain any stream"
        if lower.contains("matches no streams") || lower.contains("does not contain any stream") {
            // Soft error — caller decides whether to bubble or treat as
            // "indexed with empty content".
            return Err(EngineError::NoAudioTrack(input.display().to_string()));
        }

        // Otherwise: pass through ffmpeg's stderr, but compactly.
        let detail = compact_stderr(&stderr_text);
        return Err(EngineError::Transcribe(format!(
            "ffmpeg failed on {} (exit {:?}): {}",
            input.display(),
            output.status.code(),
            detail
        )));
    }
    if !output_path.exists() {
        return Err(EngineError::Transcribe(
            "ffmpeg produced no output WAV".to_string(),
        ));
    }
    Ok(())
}

/// Default language we hand whisper.cpp when the user hasn't set
/// `ANUBIS_WHISPER_LANGUAGE`. We default to Indonesian because:
///   * auto-detect on real-world non-English audio is empirically unreliable
///     even with the medium model — it commonly mis-classifies and falls
///     back to the meta-token `(speaking in foreign language)`.
///   * The first user of this project works with Indonesian content.
/// Set `ANUBIS_WHISPER_LANGUAGE=auto` to restore detection, or to any
/// ISO-639-1 code (`en`, `ja`, …) to force a different language.
const DEFAULT_WHISPER_LANGUAGE: &str = "id";

fn whisper_language() -> Option<String> {
    let raw = std::env::var("ANUBIS_WHISPER_LANGUAGE")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_WHISPER_LANGUAGE.to_string());
    if raw == "auto" {
        None
    } else {
        Some(raw)
    }
}

/// Pack ffmpeg's stderr into a single bounded line for error messages.
/// Drops blank lines, preserves the last few lines (closest to the failure)
/// and caps total length so a long log doesn't blow up the UI.
fn compact_stderr(raw: &str) -> String {
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return "no stderr output".to_string();
    }
    let take_last = lines.len().min(4);
    let mut joined = lines[lines.len() - take_last..].join(" / ");
    const MAX: usize = 600;
    if joined.len() > MAX {
        joined.truncate(MAX);
        joined.push('…');
    }
    joined
}

fn run_whisper(exe: &Path, model: &Path, wav: &Path) -> Result<String, EngineError> {
    // We write the transcript to a sidecar .txt via `-otxt -of <prefix>` and
    // read it back from disk. Reasons:
    //   * Avoids fighting whisper.cpp's verbose stdout decoration (model
    //     metrics, "[whisper_print_timings]" etc.).
    //   * Survives any future flag rename — older `-nt -np` flags were removed
    //     in newer whisper-cli builds and produced silent exit-1 failures with
    //     no stderr output to debug from.
    //   * Auto-language detection is the default when `-l` is omitted, so we
    //     drop that flag too.
    let prefix = wav.with_extension(""); // strip ".wav"
    let txt_path = wav.with_extension("txt");
    let _ = std::fs::remove_file(&txt_path); // start clean

    let mut command = Command::new(exe);
    command
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(wav)
        .arg("-otxt")
        .arg("-of")
        .arg(&prefix)
        // Suppress non-speech meta-tokens like `(speaking in foreign language)`,
        // `(music playing)`, `(silence)`. Useful annotations for video
        // captioning, useless noise in a search index.
        .arg("--suppress-nst");

    if let Some(lang) = whisper_language() {
        command.arg("-l").arg(&lang);
    }

    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            EngineError::Transcribe(format!("spawn whisper ({}): {error}", exe.display()))
        })?;

    if !output.status.success() {
        // Errors can land on either stream depending on whisper.cpp version
        // — fold both into the message so we never report a bare exit code.
        let mut detail_lines: Vec<String> = Vec::new();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr_compact = compact_stderr(&stderr);
        if stderr_compact != "no stderr output" {
            detail_lines.push(format!("stderr: {stderr_compact}"));
        }
        let stdout_compact = compact_stderr(&stdout);
        if stdout_compact != "no stderr output" {
            detail_lines.push(format!("stdout: {stdout_compact}"));
        }
        let detail = if detail_lines.is_empty() {
            "no output on either stream — binary may have crashed silently".to_string()
        } else {
            detail_lines.join(" || ")
        };
        return Err(EngineError::Transcribe(format!(
            "whisper exited with status {:?}: {}",
            output.status.code(),
            detail
        )));
    }

    if !txt_path.exists() {
        // Exit succeeded but no .txt written — fall back to stdout parsing.
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Ok(clean_whisper_output(&stdout));
    }
    let text = std::fs::read_to_string(&txt_path)
        .map_err(|error| EngineError::Transcribe(format!("read transcript: {error}")))?;
    let _ = std::fs::remove_file(&txt_path);
    Ok(clean_whisper_output(&text))
}

/// Strip whisper.cpp's optional bracketed timestamp prefixes and meta-token
/// annotations (`(music playing)`, `[Applause]`, etc.) — both because they
/// pollute the search index and because a transcript consisting of nothing
/// but meta-tokens means the actual ASR failed.
fn clean_whisper_output(raw: &str) -> String {
    let mut kept = Vec::new();
    for line in raw.lines() {
        let trimmed = strip_timestamp_prefix(line.trim()).trim();
        if trimmed.is_empty() || is_meta_only(trimmed) {
            continue;
        }
        kept.push(trimmed.to_string());
    }
    kept.join(" ")
}

/// A line is "meta-only" when, after stripping whitespace, every chunk is a
/// parenthesised / bracketed annotation like `(music)`, `[Applause]`,
/// `(speaking in foreign language)`. These tokens come from Whisper's
/// training on captioned video and carry no useful text.
fn is_meta_only(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    // Quick path: single parenthesised / bracketed annotation.
    let bytes = trimmed.as_bytes();
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    let wraps = (first == b'(' && last == b')') || (first == b'[' && last == b']');
    if wraps {
        return true;
    }
    // Multi-chunk path: every whitespace-separated chunk is wrapped.
    trimmed.split_whitespace().all(|chunk| {
        let b = chunk.as_bytes();
        !b.is_empty()
            && ((b[0] == b'(' && b[b.len() - 1] == b')')
                || (b[0] == b'[' && b[b.len() - 1] == b']'))
    })
}

fn strip_timestamp_prefix(line: &str) -> &str {
    // Matches `[00:00:00.000 --> 00:00:05.000]  text`. If no prefix, returns
    // the line unchanged.
    if let Some(rest) = line.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            return rest[close + 1..].trim_start();
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::{clean_whisper_output, is_meta_only};

    #[test]
    fn strips_only_metatag_lines() {
        let raw = "(speaking in foreign language)\nHalo dunia\n(music playing)\nApa kabar?";
        assert_eq!(clean_whisper_output(raw), "Halo dunia Apa kabar?");
    }

    #[test]
    fn empty_when_all_metatags() {
        let raw = "(speaking in foreign language)\n[Applause]\n(silence)";
        assert_eq!(clean_whisper_output(raw), "");
    }

    #[test]
    fn keeps_lines_that_only_mention_metatag_words() {
        // "music" inside a real sentence must NOT be stripped — only fully
        // wrapped `(...)` / `[...]` lines are noise.
        assert!(!is_meta_only("Mereka mendengarkan music di kafe"));
    }
}

/// Public entry point — transcribe any ffmpeg-supported file to plain text.
/// Side effects (configurable via env vars):
///   * Writes `<source-stem>.txt` next to the source file.
///   * Writes `<source-stem>.wav` next to the source file.
///   * Both can be redirected with `ANUBIS_TRANSCRIPT_DIR` (folder where the
///     companion files are written instead of the source folder).
///   * `ANUBIS_KEEP_WAV=0` skips writing the WAV.
pub fn transcribe_file(path: &Path) -> Result<String, EngineError> {
    let artifacts = artifacts()?;
    let output_dir = resolve_output_dir(path);
    if let Err(error) = std::fs::create_dir_all(&output_dir) {
        return Err(EngineError::Transcribe(format!(
            "create output dir {output_dir:?}: {error}"
        )));
    }

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("transcript");
    // `.anubis.` infix marks files as engine-generated outputs so the indexer
    // can skip them on the next pass — otherwise the sidecar wav would be
    // re-fed to ffmpeg in a loop ("Output same as Input #0 — exiting").
    let final_wav = output_dir.join(format!("{stem}.anubis.wav"));
    let final_txt = output_dir.join(format!("{stem}.anubis.txt"));

    // Extract audio straight to the final wav path so we don't have to round
    // -trip through a temp file when the user wants to keep it.
    extract_audio_to_wav(&artifacts.ffmpeg_exe, path, &final_wav)?;

    let text = run_whisper(&artifacts.whisper_exe, &artifacts.whisper_model, &final_wav)?;

    // After clean_whisper_output drops timestamp prefixes and meta-tokens,
    // a truly-empty result is the canonical signal that ASR failed (model
    // emitted only `(speaking in foreign language)` or similar). Surface
    // that so the user knows to try a different language hint.
    if text.trim().is_empty() {
        tracing::warn!(
            "transcript empty for {} — whisper produced only non-speech tokens. \
             Try `set ANUBIS_WHISPER_LANGUAGE=auto` or a more specific ISO-639-1 code, \
             or upgrade the model with `set ANUBIS_WHISPER_MODEL=large-v3`.",
            path.display()
        );
    }

    if let Err(error) = std::fs::write(&final_txt, &text) {
        // Non-fatal — transcript still goes to the index even if writing the
        // sidecar .txt fails (e.g. read-only folder).
        tracing::warn!("failed to write transcript {final_txt:?}: {error}");
    } else {
        tracing::info!("wrote transcript {}", final_txt.display());
    }

    if !keep_wav() {
        let _ = std::fs::remove_file(&final_wav);
    } else {
        tracing::info!("kept extracted audio {}", final_wav.display());
    }

    Ok(text)
}

/// Where to put the `<name>.txt` and `<name>.wav` companions. Defaults to the
/// folder containing the source file, override with `ANUBIS_TRANSCRIPT_DIR`.
fn resolve_output_dir(source: &Path) -> PathBuf {
    if let Ok(env_dir) = std::env::var("ANUBIS_TRANSCRIPT_DIR") {
        if !env_dir.trim().is_empty() {
            return PathBuf::from(env_dir);
        }
    }
    source
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn keep_wav() -> bool {
    // Default ON per user preference; opt-out only when explicitly set to a
    // falsy value.
    match std::env::var("ANUBIS_KEEP_WAV").as_deref() {
        Ok("0") | Ok("false") | Ok("no") | Ok("off") => false,
        _ => true,
    }
}
