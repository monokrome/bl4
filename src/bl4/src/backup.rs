//! Smart backup management with hash tracking.
//!
//! This module handles automatic backup creation with intelligent detection
//! of when backups should be created or preserved.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Metadata tracking save file hashes for smart backup management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Hash of the save file when the backup was created
    pub original_hash: String,

    /// Hash of the save file after the last edit
    pub last_edit_hash: String,
}

impl BackupMetadata {
    /// Create new metadata from a hash
    pub fn new(hash: String) -> Self {
        BackupMetadata {
            original_hash: hash.clone(),
            last_edit_hash: hash,
        }
    }

    /// Update the last edit hash
    pub fn update_last_edit(&mut self, hash: String) {
        self.last_edit_hash = hash;
    }
}

/// Compute SHA-256 hash of a file
pub fn hash_file(path: &Path) -> Result<String, BackupError> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Get paths for backup and metadata files
pub fn backup_paths(save_path: &Path) -> (PathBuf, PathBuf) {
    let backup_path = save_path.with_extension("sav.bak");
    let metadata_path = save_path.with_extension("sav.bak.json");
    (backup_path, metadata_path)
}

/// Read backup metadata if it exists
pub fn read_metadata(metadata_path: &Path) -> Result<Option<BackupMetadata>, BackupError> {
    if !metadata_path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(metadata_path)?;
    let metadata: BackupMetadata = serde_json::from_str(&data)?;
    Ok(Some(metadata))
}

/// Write backup metadata
pub fn write_metadata(metadata_path: &Path, metadata: &BackupMetadata) -> Result<(), BackupError> {
    let json = serde_json::to_string_pretty(metadata)?;
    fs::write(metadata_path, json)?;
    Ok(())
}

/// Determine if we should create a new backup
///
/// Returns true if:
/// - No backup exists
/// - Current save file hash doesn't match any tracked hashes (user replaced the file)
///
/// Returns false if:
/// - Current save matches last_edit_hash (we already edited this)
/// - Current save matches original_hash (user restored from backup)
pub fn should_create_backup(
    save_path: &Path,
    backup_path: &Path,
    metadata_path: &Path,
) -> Result<bool, BackupError> {
    // If no backup exists, we should create one
    if !backup_path.exists() {
        return Ok(true);
    }

    // If backup exists but no metadata, be safe and don't overwrite
    let Some(metadata) = read_metadata(metadata_path)? else {
        return Ok(false);
    };

    // Get current save file hash
    let current_hash = hash_file(save_path)?;

    // If current hash matches either tracked hash, don't create new backup
    if current_hash == metadata.original_hash || current_hash == metadata.last_edit_hash {
        return Ok(false);
    }

    // Current hash doesn't match - user must have replaced the save file
    Ok(true)
}

/// Create a backup and initialize metadata
pub fn create_backup(
    save_path: &Path,
    backup_path: &Path,
    metadata_path: &Path,
) -> Result<(), BackupError> {
    // Copy save file to backup
    fs::copy(save_path, backup_path)?;

    // Compute hash and create metadata
    let hash = hash_file(save_path)?;
    let metadata = BackupMetadata::new(hash);
    write_metadata(metadata_path, &metadata)?;

    Ok(())
}

/// Update metadata after editing a save file
pub fn update_after_edit(save_path: &Path, metadata_path: &Path) -> Result<(), BackupError> {
    let current_hash = hash_file(save_path)?;

    // Read existing metadata (or create new if missing)
    let mut metadata =
        read_metadata(metadata_path)?.unwrap_or_else(|| BackupMetadata::new(current_hash.clone()));

    // Update last edit hash
    metadata.update_last_edit(current_hash);
    write_metadata(metadata_path, &metadata)?;

    Ok(())
}

/// Perform smart backup if needed
///
/// This is the main entry point for backup management:
/// - Checks if backup should be created based on hash tracking
/// - Creates backup if needed
/// - Returns true if a new backup was created
pub fn smart_backup(save_path: &Path) -> Result<bool, BackupError> {
    let (backup_path, metadata_path) = backup_paths(save_path);

    if should_create_backup(save_path, &backup_path, &metadata_path)? {
        create_backup(save_path, &backup_path, &metadata_path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_file(path: &Path, content: &[u8]) -> Result<(), BackupError> {
        let mut file = fs::File::create(path)?;
        file.write_all(content)?;
        Ok(())
    }

    #[test]
    fn test_hash_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.sav");

        create_test_file(&file_path, b"test content").unwrap();
        let hash = hash_file(&file_path).unwrap();

        // SHA-256 of "test content"
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 is 32 bytes = 64 hex chars
    }

    #[test]
    fn test_backup_paths() {
        let save_path = Path::new("/tmp/1.sav");
        let (backup, metadata) = backup_paths(save_path);

        assert_eq!(backup, PathBuf::from("/tmp/1.sav.bak"));
        assert_eq!(metadata, PathBuf::from("/tmp/1.sav.bak.json"));
    }

    #[test]
    fn test_metadata_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let metadata_path = temp_dir.path().join("test.json");

        let original = BackupMetadata::new("hash123".to_string());
        write_metadata(&metadata_path, &original).unwrap();

        let loaded = read_metadata(&metadata_path).unwrap().unwrap();
        assert_eq!(loaded.original_hash, "hash123");
        assert_eq!(loaded.last_edit_hash, "hash123");
    }

    #[test]
    fn test_should_create_backup_no_backup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_path = temp_dir.path().join("test.sav");
        let (backup_path, metadata_path) = backup_paths(&save_path);

        create_test_file(&save_path, b"content").unwrap();

        let should_backup = should_create_backup(&save_path, &backup_path, &metadata_path).unwrap();
        assert!(should_backup);
    }

    #[test]
    fn test_should_create_backup_same_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_path = temp_dir.path().join("test.sav");
        let (backup_path, metadata_path) = backup_paths(&save_path);

        create_test_file(&save_path, b"content").unwrap();
        create_backup(&save_path, &backup_path, &metadata_path).unwrap();

        // Same file shouldn't need new backup
        let should_backup = should_create_backup(&save_path, &backup_path, &metadata_path).unwrap();
        assert!(!should_backup);
    }

    #[test]
    fn test_should_create_backup_after_edit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_path = temp_dir.path().join("test.sav");
        let (backup_path, metadata_path) = backup_paths(&save_path);

        create_test_file(&save_path, b"original").unwrap();
        create_backup(&save_path, &backup_path, &metadata_path).unwrap();

        // Modify the file
        create_test_file(&save_path, b"modified").unwrap();
        update_after_edit(&save_path, &metadata_path).unwrap();

        // Should not create new backup (we track the edit)
        let should_backup = should_create_backup(&save_path, &backup_path, &metadata_path).unwrap();
        assert!(!should_backup);
    }

    #[test]
    fn test_should_create_backup_replaced_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_path = temp_dir.path().join("test.sav");
        let (backup_path, metadata_path) = backup_paths(&save_path);

        create_test_file(&save_path, b"original").unwrap();
        create_backup(&save_path, &backup_path, &metadata_path).unwrap();

        // Simulate user replacing save with different file
        create_test_file(&save_path, b"completely different content").unwrap();

        // Should create new backup
        let should_backup = should_create_backup(&save_path, &backup_path, &metadata_path).unwrap();
        assert!(should_backup);
    }

    #[test]
    fn test_smart_backup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let save_path = temp_dir.path().join("test.sav");

        create_test_file(&save_path, b"content").unwrap();

        // First backup should be created
        let created = smart_backup(&save_path).unwrap();
        assert!(created);

        // Second backup should be skipped (same file)
        let created = smart_backup(&save_path).unwrap();
        assert!(!created);
    }
}
