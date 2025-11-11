//! Smart backup management with hash tracking.
//!
//! This module handles automatic backup creation with intelligent detection
//! of when backups should be created or preserved.

use blake3;
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

    #[error("Save decryption error: {0}")]
    SaveError(#[from] crate::save::SaveError),

    #[error("Backup version not found: {0}")]
    VersionNotFound(String),
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

/// A single backup version with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVersion {
    /// Unique identifier for this backup version
    pub id: String,

    /// Timestamp when backup was created (RFC3339 format)
    pub timestamp: String,

    /// SHA-256 hash of the encrypted .sav file
    pub file_hash_sha256: String,

    /// BLAKE3 hash of the encrypted .sav file (for collision resistance)
    pub file_hash_blake3: String,

    /// SHA-256 hash of the decrypted YAML content
    pub content_hash_sha256: Option<String>,

    /// BLAKE3 hash of the decrypted YAML content
    pub content_hash_blake3: Option<String>,

    /// Optional user-provided tag/label
    pub tag: Option<String>,

    /// Optional user-provided description
    pub description: Option<String>,

    /// Whether this was auto-created or manually created
    pub auto_created: bool,

    /// File size in bytes
    pub file_size: u64,
}

/// Metadata for versioned backup system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedBackupMetadata {
    /// List of all backup versions, sorted by timestamp (newest first)
    pub versions: Vec<BackupVersion>,

    /// Maximum number of auto-created versions to keep (None = unlimited)
    pub max_auto_versions: Option<usize>,
}

impl Default for VersionedBackupMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionedBackupMetadata {
    /// Create new empty metadata
    pub fn new() -> Self {
        VersionedBackupMetadata {
            versions: Vec::new(),
            max_auto_versions: Some(10), // Default to keeping 10 auto-backups
        }
    }

    /// Add a new version and sort by timestamp
    pub fn add_version(&mut self, version: BackupVersion) {
        self.versions.push(version);
        // Sort newest first
        self.versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    /// Get a version by ID
    pub fn get_version(&self, id: &str) -> Option<&BackupVersion> {
        self.versions.iter().find(|v| v.id == id)
    }

    /// Remove a version by ID
    pub fn remove_version(&mut self, id: &str) -> bool {
        if let Some(pos) = self.versions.iter().position(|v| v.id == id) {
            self.versions.remove(pos);
            true
        } else {
            false
        }
    }

    /// Clean up old auto-created versions beyond max_auto_versions
    pub fn cleanup_old_auto_versions(&mut self) -> Vec<String> {
        let Some(max) = self.max_auto_versions else {
            return Vec::new();
        };

        let auto_versions: Vec<_> = self
            .versions
            .iter()
            .filter(|v| v.auto_created)
            .cloned()
            .collect();

        if auto_versions.len() <= max {
            return Vec::new();
        }

        // Keep newest max versions, remove the rest
        let to_remove: Vec<String> = auto_versions
            .iter()
            .skip(max)
            .map(|v| v.id.clone())
            .collect();

        for id in &to_remove {
            self.remove_version(id);
        }

        to_remove
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

// ============================================================================
// Versioned Backup System
// ============================================================================

/// Compute dual hashes (SHA-256 and BLAKE3) of file data
pub fn compute_dual_hashes(data: &[u8]) -> (String, String) {
    // SHA-256
    let mut sha256 = Sha256::new();
    sha256.update(data);
    let sha256_hash = hex::encode(sha256.finalize());

    // BLAKE3
    let blake3_hash = blake3::hash(data).to_hex().to_string();

    (sha256_hash, blake3_hash)
}

/// Get the backup directory path for a save file
pub fn versioned_backup_dir(save_path: &Path) -> PathBuf {
    let mut dir = save_path.to_path_buf();
    let filename = dir.file_name().unwrap().to_str().unwrap();
    dir.set_file_name(format!("{}.backups", filename));
    dir
}

/// Get the metadata path for versioned backups
pub fn versioned_metadata_path(save_path: &Path) -> PathBuf {
    versioned_backup_dir(save_path).join("metadata.json")
}

/// Read versioned backup metadata
pub fn read_versioned_metadata(save_path: &Path) -> Result<VersionedBackupMetadata, BackupError> {
    let metadata_path = versioned_metadata_path(save_path);

    if !metadata_path.exists() {
        return Ok(VersionedBackupMetadata::new());
    }

    let data = fs::read_to_string(&metadata_path)?;
    let metadata: VersionedBackupMetadata = serde_json::from_str(&data)?;
    Ok(metadata)
}

/// Write versioned backup metadata
pub fn write_versioned_metadata(
    save_path: &Path,
    metadata: &VersionedBackupMetadata,
) -> Result<(), BackupError> {
    let backup_dir = versioned_backup_dir(save_path);
    fs::create_dir_all(&backup_dir)?;

    let metadata_path = versioned_metadata_path(save_path);
    let json = serde_json::to_string_pretty(metadata)?;
    fs::write(&metadata_path, json)?;
    Ok(())
}

/// Create a versioned backup
pub fn create_versioned_backup(
    save_path: &Path,
    steam_id: Option<&str>,
    tag: Option<String>,
    description: Option<String>,
    auto_created: bool,
) -> Result<BackupVersion, BackupError> {
    // Generate unique ID
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Read and hash the file
    let file_data = fs::read(save_path)?;
    let (file_hash_sha256, file_hash_blake3) = compute_dual_hashes(&file_data);
    let file_size = file_data.len() as u64;

    // Optionally compute content hashes (if we can decrypt)
    let (content_hash_sha256, content_hash_blake3) = if let Some(steam_id) = steam_id {
        match crate::decrypt_sav(&file_data, steam_id) {
            Ok(yaml) => {
                let (sha256, blake3) = compute_dual_hashes(&yaml);
                (Some(sha256), Some(blake3))
            }
            Err(_) => (None, None), // Can't decrypt, skip content hashes
        }
    } else {
        (None, None)
    };

    // Create backup directory
    let backup_dir = versioned_backup_dir(save_path);
    fs::create_dir_all(&backup_dir)?;

    // Copy file to backup directory
    let backup_file_path = backup_dir.join(format!("{}.sav", id));
    fs::copy(save_path, &backup_file_path)?;

    // Create version metadata
    let version = BackupVersion {
        id: id.clone(),
        timestamp,
        file_hash_sha256,
        file_hash_blake3,
        content_hash_sha256,
        content_hash_blake3,
        tag,
        description,
        auto_created,
        file_size,
    };

    // Update metadata
    let mut metadata = read_versioned_metadata(save_path)?;
    metadata.add_version(version.clone());

    // Cleanup old auto-backups if needed
    if auto_created {
        let removed_ids = metadata.cleanup_old_auto_versions();
        for old_id in removed_ids {
            let old_path = backup_dir.join(format!("{}.sav", old_id));
            let _ = fs::remove_file(old_path); // Ignore errors
        }
    }

    write_versioned_metadata(save_path, &metadata)?;

    Ok(version)
}

/// Check if a versioned backup should be created (based on dual hashes)
pub fn should_create_versioned_backup(
    save_path: &Path,
    steam_id: Option<&str>,
) -> Result<bool, BackupError> {
    let metadata = read_versioned_metadata(save_path)?;

    if metadata.versions.is_empty() {
        return Ok(true);
    }

    // Read current file and compute hashes
    let file_data = fs::read(save_path)?;
    let (file_sha256, file_blake3) = compute_dual_hashes(&file_data);

    // Check if current hashes match any existing backup
    for version in &metadata.versions {
        if version.file_hash_sha256 == file_sha256 && version.file_hash_blake3 == file_blake3 {
            return Ok(false); // Already have this exact file backed up
        }
    }

    // If we have steam_id, also check content hashes
    if let Some(steam_id) = steam_id {
        if let Ok(yaml) = crate::decrypt_sav(&file_data, steam_id) {
            let (content_sha256, content_blake3) = compute_dual_hashes(&yaml);

            for version in &metadata.versions {
                if let (Some(v_sha256), Some(v_blake3)) =
                    (&version.content_hash_sha256, &version.content_hash_blake3)
                {
                    if *v_sha256 == content_sha256 && *v_blake3 == content_blake3 {
                        return Ok(false); // Same content, don't need backup
                    }
                }
            }
        }
    }

    Ok(true) // Hashes don't match any existing backup
}

/// List all backup versions for a save file
pub fn list_backup_versions(save_path: &Path) -> Result<Vec<BackupVersion>, BackupError> {
    let metadata = read_versioned_metadata(save_path)?;
    Ok(metadata.versions)
}

/// Restore a specific backup version
pub fn restore_backup_version(save_path: &Path, version_id: &str) -> Result<(), BackupError> {
    let backup_dir = versioned_backup_dir(save_path);
    let backup_file = backup_dir.join(format!("{}.sav", version_id));

    if !backup_file.exists() {
        return Err(BackupError::VersionNotFound(version_id.to_string()));
    }

    // Verify the backup exists in metadata
    let metadata = read_versioned_metadata(save_path)?;
    if metadata.get_version(version_id).is_none() {
        return Err(BackupError::VersionNotFound(version_id.to_string()));
    }

    // Copy backup to save location
    fs::copy(&backup_file, save_path)?;
    Ok(())
}

/// Delete a specific backup version
pub fn delete_backup_version(save_path: &Path, version_id: &str) -> Result<(), BackupError> {
    let backup_dir = versioned_backup_dir(save_path);
    let backup_file = backup_dir.join(format!("{}.sav", version_id));

    // Remove from metadata
    let mut metadata = read_versioned_metadata(save_path)?;
    if !metadata.remove_version(version_id) {
        return Err(BackupError::VersionNotFound(version_id.to_string()));
    }
    write_versioned_metadata(save_path, &metadata)?;

    // Delete backup file
    if backup_file.exists() {
        fs::remove_file(&backup_file)?;
    }

    Ok(())
}

/// Update tag and description for a backup version
pub fn update_backup_version_metadata(
    save_path: &Path,
    version_id: &str,
    tag: Option<String>,
    description: Option<String>,
) -> Result<(), BackupError> {
    let mut metadata = read_versioned_metadata(save_path)?;

    // Find the version and update it
    let version = metadata
        .versions
        .iter_mut()
        .find(|v| v.id == version_id)
        .ok_or_else(|| BackupError::VersionNotFound(version_id.to_string()))?;

    version.tag = tag;
    version.description = description;

    write_versioned_metadata(save_path, &metadata)?;
    Ok(())
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
