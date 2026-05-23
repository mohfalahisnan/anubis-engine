//! Runtime-tunable engine settings, persisted as key/value rows in the
//! existing `index_stats` table.
//!
//! Why a tiny module: parsers (e.g. video.rs) need to know whether
//! transcription is enabled, but they don't carry an `AppState`. A global
//! `AtomicBool` keeps the lookup zero-cost and synchronous, while the
//! Tauri command writes both the in-memory flag AND the SQLite row so the
//! choice survives a restart.

use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;

use crate::EngineError;

const KEY_TRANSCRIPTION_ENABLED: &str = "transcription_enabled";
const DEFAULT_TRANSCRIPTION_ENABLED: bool = true;

static TRANSCRIPTION_ENABLED: AtomicBool = AtomicBool::new(DEFAULT_TRANSCRIPTION_ENABLED);

pub fn transcription_enabled() -> bool {
    TRANSCRIPTION_ENABLED.load(Ordering::Relaxed)
}

pub fn set_transcription_enabled(value: bool) {
    TRANSCRIPTION_ENABLED.store(value, Ordering::Relaxed);
}

/// Read the persisted value from `index_stats` and hydrate the in-memory
/// flag. Called once at engine startup.
pub fn load_from_db(conn: &Connection) -> Result<(), EngineError> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM index_stats WHERE key = ?1",
            [KEY_TRANSCRIPTION_ENABLED],
            |row| row.get(0),
        )
        .ok();
    if let Some(raw) = stored {
        let parsed = matches!(raw.as_str(), "1" | "true" | "yes" | "on");
        TRANSCRIPTION_ENABLED.store(parsed, Ordering::Relaxed);
    }
    Ok(())
}

/// Write the value to both the in-memory flag and the DB so a restart
/// keeps the user's choice.
pub fn persist(conn: &Connection, value: bool) -> Result<(), EngineError> {
    set_transcription_enabled(value);
    conn.execute(
        "INSERT INTO index_stats (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [
            KEY_TRANSCRIPTION_ENABLED,
            if value { "true" } else { "false" },
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::migrate;

    #[test]
    fn round_trips_through_sqlite() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        // Default is true.
        assert!(transcription_enabled());

        persist(&conn, false).unwrap();
        assert!(!transcription_enabled());

        // Wipe in-memory, reload from DB, value should persist.
        TRANSCRIPTION_ENABLED.store(true, Ordering::Relaxed);
        load_from_db(&conn).unwrap();
        assert!(!transcription_enabled());
    }
}
