use bl4::SaveFile;
use bl4_idb::SqliteDb;
use std::path::PathBuf;
use std::sync::Mutex;

/// Represents a loaded save file with metadata.
pub struct LoadedSave {
    pub path: PathBuf,
    pub save: SaveFile,
    pub is_profile: bool,
    pub steam_id: String,
    pub modified: bool,
}

/// Shared application state.
pub struct AppState {
    pub current_save: Mutex<Option<LoadedSave>>,
    pub items_db: Mutex<Option<SqliteDb>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_save: Mutex::new(None),
            items_db: Mutex::new(None),
        }
    }
}
