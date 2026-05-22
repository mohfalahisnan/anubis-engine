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
//!   1. ffmpeg (via ffmpeg-sidecar) decodes the source file into 16 kHz mono
//!      f32 PCM streamed to a temp WAV.
//!   2. whisper.cpp CLI transcribes the WAV. Output is parsed from stdout —
//!      each segment line looks like `[hh:mm:ss.SSS --> hh:mm:ss.SSS]  text`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use ffmpeg_sidecar::command::FfmpegCommand;

use crate::engine::download::ensure_file;
use crate::engine::events;
use crate::EngineError;

/// Pinned to a release that's known to ship `whisper-bin-x64.zip`.
const WHISPER_BIN_VERSION: &str = "v1.7.6";
const WHISPER_BIN_URL_WIN64: &str =
    "https://github.com/ggerganov/whisper.cpp/releases/download/v1.7.6/whisper-bin-x64.zip";

const WHISPER_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
const WHISPER_MODEL_FILE: &str = "ggml-base.bin";

const WHISPER_DIR_NAME: &str = "transcription-whisper";
const FFMPEG_EVENT_ID: &str = "ffmpeg";
const FFMPEG_LABEL: &str = "ffmpeg (audio extractor)";
const WHISPER_BIN_EVENT_ID: &str = "whisper-binary";
const WHISPER_BIN_LABEL: &str = "Whisper.cpp binary";
const WHISPER_MODEL_EVENT_ID: &str = "whisper-model";
const WHISPER_MODEL_LABEL: &str = "Whisper model (base multilingual ~145MB)";

static MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();
static ARTIFACTS: OnceLock<Result<Artifacts, String>> = OnceLock::new();

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
    whisper_exe: PathBuf,
    whisper_model: PathBuf,
}

fn artifacts() -> Result<&'static Artifacts, EngineError> {
    ARTIFACTS
        .get_or_init(init_artifacts)
        .as_ref()
        .map_err(|message| EngineError::Transcribe(message.clone()))
}

fn init_artifacts() -> Result<Artifacts, String> {
    // ffmpeg first — auto_download has its own progress UI on stderr, but we
    // wrap it with start/ready events so the UI banner shows what's happening.
    events::emit_starting(FFMPEG_EVENT_ID, FFMPEG_LABEL);
    if let Err(error) = ensure_ffmpeg_present() {
        events::emit_error(FFMPEG_EVENT_ID, FFMPEG_LABEL, &error);
        return Err(error);
    }
    events::emit_ready(FFMPEG_EVENT_ID, FFMPEG_LABEL);

    // whisper.cpp binary (Windows only for now — see WHISPER_BIN_URL_WIN64).
    let whisper_dir = models_dir().join(WHISPER_DIR_NAME);
    std::fs::create_dir_all(&whisper_dir)
        .map_err(|error| format!("create whisper dir: {error}"))?;
    let whisper_exe = ensure_whisper_binary(&whisper_dir)?;
    let model_path = whisper_dir.join(WHISPER_MODEL_FILE);
    ensure_file(
        &model_path,
        WHISPER_MODEL_URL,
        WHISPER_MODEL_EVENT_ID,
        WHISPER_MODEL_LABEL,
    )?;

    Ok(Artifacts {
        whisper_exe,
        whisper_model: model_path,
    })
}

fn ensure_ffmpeg_present() -> Result<(), String> {
    use ffmpeg_sidecar::download::auto_download;
    if ffmpeg_sidecar::command::ffmpeg_is_installed() {
        return Ok(());
    }
    auto_download().map_err(|error| format!("ffmpeg auto_download failed: {error}"))
}

#[cfg(target_os = "windows")]
fn ensure_whisper_binary(dir: &Path) -> Result<PathBuf, String> {
    // Whisper.cpp Win64 release ships a zip with main.exe + DLLs that must
    // live in the same directory. Extract only on first run.
    let marker = dir.join(format!(".whisper-{WHISPER_BIN_VERSION}"));
    let exe = dir.join("main.exe");
    if exe.exists() && marker.exists() {
        return Ok(exe);
    }

    let zip_path = dir.join("whisper-bin-x64.zip");
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

    if !exe.exists() {
        // Some releases name the binary differently — fall back to whisper-cli.exe.
        let alt = dir.join("whisper-cli.exe");
        if alt.exists() {
            std::fs::copy(&alt, &exe)
                .map_err(|error| format!("copy whisper-cli.exe -> main.exe: {error}"))?;
        } else {
            return Err(format!(
                "whisper binary not found after extract in {dir:?}; expected main.exe or whisper-cli.exe"
            ));
        }
    }

    std::fs::write(&marker, WHISPER_BIN_VERSION)
        .map_err(|error| format!("write version marker: {error}"))?;
    Ok(exe)
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
    let file = std::fs::File::open(zip_path)
        .map_err(|error| format!("open whisper zip: {error}"))?;
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
/// WAV file in `temp_dir`. Returns the path to the WAV — caller cleans up.
fn extract_audio_to_wav(input: &Path, temp_dir: &Path) -> Result<PathBuf, EngineError> {
    let wav_path = temp_dir.join(format!("anubis-{}.wav", uuid::Uuid::new_v4()));
    let output_arg = wav_path.to_string_lossy().to_string();

    let mut command = FfmpegCommand::new();
    command
        .input(input.to_string_lossy().as_ref())
        .args([
            "-vn", // drop video stream
            "-ac", "1", // mono
            "-ar", "16000", // Whisper expects 16 kHz
            "-c:a", "pcm_s16le",
            "-y", // overwrite
        ])
        .output(output_arg);

    let mut child = command
        .spawn()
        .map_err(|error| EngineError::Transcribe(format!("spawn ffmpeg: {error}")))?;
    let status = child
        .wait()
        .map_err(|error| EngineError::Transcribe(format!("wait ffmpeg: {error}")))?;
    if !status.success() {
        return Err(EngineError::Transcribe(format!(
            "ffmpeg exited with status {:?}",
            status.code()
        )));
    }
    if !wav_path.exists() {
        return Err(EngineError::Transcribe(
            "ffmpeg produced no output WAV".to_string(),
        ));
    }
    Ok(wav_path)
}

fn run_whisper(exe: &Path, model: &Path, wav: &Path) -> Result<String, EngineError> {
    // -nt = no timestamps (cleaner output), -np = no prints,
    // -l auto = autodetect language.
    let output = Command::new(exe)
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(wav)
        .args(["-l", "auto", "-nt", "-np"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| EngineError::Transcribe(format!("spawn whisper: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::Transcribe(format!(
            "whisper exited with status {:?}: {}",
            output.status.code(),
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(clean_whisper_output(&stdout))
}

/// Strip whisper.cpp's optional bracketed timestamp prefixes and join lines.
fn clean_whisper_output(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let trimmed = strip_timestamp_prefix(line.trim());
        if trimmed.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(trimmed);
    }
    out
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

/// Public entry point — transcribe any ffmpeg-supported file to plain text.
pub fn transcribe_file(path: &Path) -> Result<String, EngineError> {
    let artifacts = artifacts()?;
    let temp_dir = std::env::temp_dir();
    let wav = extract_audio_to_wav(path, &temp_dir)?;
    let result = run_whisper(&artifacts.whisper_exe, &artifacts.whisper_model, &wav);
    // Best-effort cleanup; transcription succeeded either way.
    let _ = std::fs::remove_file(&wav);
    result
}

