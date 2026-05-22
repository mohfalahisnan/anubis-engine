//! Lightweight Tauri event emitter for long-running setup steps the user
//! would otherwise stare at a blank UI for (model downloads, first-time
//! initialisation). Holds the [`tauri::AppHandle`] in a `OnceLock` so leaf
//! modules (e.g. the OCR engine) can emit without plumbing it through every
//! call.

use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EVENT_MODEL_DOWNLOAD: &str = "model-download";

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn set_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

fn app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Starting,
    Downloading,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelDownloadEvent {
    /// Stable id (e.g. `ocr-detection`, `ocr-recognition`, `embedding`).
    pub id: String,
    /// Human-readable label for the toast / banner.
    pub label: String,
    pub status: DownloadStatus,
    /// Bytes received so far (only meaningful while downloading).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_done: Option<u64>,
    /// Bytes total reported by the server (only meaningful while downloading).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub fn emit_model_event(event: ModelDownloadEvent) {
    let Some(handle) = app_handle() else {
        return;
    };
    if let Err(error) = handle.emit(EVENT_MODEL_DOWNLOAD, &event) {
        tracing::warn!("failed to emit model-download event: {error}");
    }
}

pub fn emit_starting(id: &str, label: &str) {
    emit_model_event(ModelDownloadEvent {
        id: id.to_string(),
        label: label.to_string(),
        status: DownloadStatus::Starting,
        bytes_done: None,
        bytes_total: None,
        message: None,
    });
}

pub fn emit_downloading(id: &str, label: &str, bytes_done: u64, bytes_total: Option<u64>) {
    emit_model_event(ModelDownloadEvent {
        id: id.to_string(),
        label: label.to_string(),
        status: DownloadStatus::Downloading,
        bytes_done: Some(bytes_done),
        bytes_total,
        message: None,
    });
}

pub fn emit_ready(id: &str, label: &str) {
    emit_model_event(ModelDownloadEvent {
        id: id.to_string(),
        label: label.to_string(),
        status: DownloadStatus::Ready,
        bytes_done: None,
        bytes_total: None,
        message: None,
    });
}

pub fn emit_error(id: &str, label: &str, message: impl Into<String>) {
    emit_model_event(ModelDownloadEvent {
        id: id.to_string(),
        label: label.to_string(),
        status: DownloadStatus::Error,
        bytes_done: None,
        bytes_total: None,
        message: Some(message.into()),
    });
}
