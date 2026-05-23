//! Per-workdir identity, hashing, and error types. A workdir is identified
//! by its canonical absolute filesystem path; storage on disk is keyed by
//! the first 16 hex chars of sha256(canonical_path).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum WorkdirError {
    #[error("Workdir not found or not a directory: {path}")]
    NotFound { path: String },
    #[error("Workdir path is not absolute or could not be canonicalised ({path}): {source}")]
    NotCanonical {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to initialise storage for workdir {id} at {path}: {source}")]
    StorageInit {
        id: String,
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Stable 16-hex-char identifier derived from sha256(canonical_path).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkdirId(String);

impl WorkdirId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkdirId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkdirInfo {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub last_used: String,
    pub doc_count: Option<i64>,
}

/// True if `candidate` resolves to a path inside (or equal to) `parent`.
/// Both inputs must already be canonicalised. Returns false if either path
/// has no prefix relationship to the other.
pub fn is_inside(candidate: &Path, parent: &Path) -> bool {
    candidate == parent || candidate.starts_with(parent)
}

/// Validate and canonicalise a workdir path, then return the canonical path
/// and a stable 16-hex-char id. Returns `NotFound` if the path doesn't exist
/// or isn't a directory; returns `NotCanonical` on permission errors / broken
/// symlinks / relative-path inputs that fail to resolve.
pub fn resolve(input: &str) -> Result<(PathBuf, WorkdirId), WorkdirError> {
    let raw = Path::new(input);
    if !raw.exists() {
        return Err(WorkdirError::NotFound {
            path: input.to_string(),
        });
    }
    if !raw.is_dir() {
        return Err(WorkdirError::NotFound {
            path: input.to_string(),
        });
    }
    let canonical =
        std::fs::canonicalize(raw).map_err(|source| WorkdirError::NotCanonical {
            path: input.to_string(),
            source,
        })?;

    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    let id = WorkdirId(hex[..16].to_string());
    Ok((canonical, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_stable_id_for_same_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (_, id_a) = resolve(dir.path().to_str().expect("utf8")).expect("resolve");
        let (_, id_b) = resolve(dir.path().to_str().expect("utf8")).expect("resolve");
        assert_eq!(id_a, id_b);
        assert_eq!(id_a.as_str().len(), 16);
    }

    #[test]
    fn resolve_rejects_nonexistent_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("does-not-exist");
        let result = resolve(missing.to_str().expect("utf8"));
        assert!(matches!(result, Err(WorkdirError::NotFound { .. })));
    }

    #[test]
    fn resolve_rejects_file_not_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "hi").expect("write");
        let result = resolve(file.to_str().expect("utf8"));
        assert!(matches!(result, Err(WorkdirError::NotFound { .. })));
    }
}
