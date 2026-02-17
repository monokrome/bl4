//! Repository trait for items database operations.
//!
//! This trait defines the interface for all database backends.

use crate::types::*;
use std::collections::HashMap;

/// Error type for repository operations
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("Item not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
}

/// Result type for repository operations
pub type RepoResult<T> = Result<T, RepoError>;

/// Trait for items database operations (synchronous version for CLI)
pub trait ItemsRepository {
    /// Initialize the database schema
    fn init(&self) -> RepoResult<()>;

    // === Items CRUD ===

    /// Add a new item with just its serial
    fn add_item(&self, serial: &str) -> RepoResult<()>;

    /// Get an item by serial
    fn get_item(&self, serial: &str) -> RepoResult<Option<Item>>;

    /// Update item metadata
    fn update_item(&self, serial: &str, update: &ItemUpdate) -> RepoResult<()>;

    /// List items with optional filters
    fn list_items(&self, filter: &ItemFilter) -> RepoResult<Vec<Item>>;

    /// Delete an item
    fn delete_item(&self, serial: &str) -> RepoResult<bool>;

    // === Verification ===

    /// Set verification status for an item
    fn set_verification_status(
        &self,
        serial: &str,
        status: VerificationStatus,
        notes: Option<&str>,
    ) -> RepoResult<()>;

    /// Set legal status for an item
    fn set_legal(&self, serial: &str, legal: bool) -> RepoResult<()>;

    /// Set legal status for all items
    fn set_all_legal(&self, legal: bool) -> RepoResult<usize>;

    // === Metadata ===

    /// Set item type
    fn set_item_type(&self, serial: &str, item_type: &str) -> RepoResult<()>;

    /// Set source for an item
    fn set_source(&self, serial: &str, source: &str) -> RepoResult<()>;

    /// Set source for items without one
    fn set_source_for_null(&self, source: &str) -> RepoResult<usize>;

    /// Set source for items matching a condition (SQL WHERE clause)
    /// WARNING: condition is inserted directly into SQL - do not use with untrusted input
    fn set_source_where(&self, source: &str, condition: &str) -> RepoResult<usize>;

    // === Parts ===

    /// Get parts for an item
    fn get_parts(&self, serial: &str) -> RepoResult<Vec<ItemPart>>;

    /// Replace all parts for an item (delete existing + insert new)
    fn set_parts(&self, serial: &str, parts: &[NewItemPart]) -> RepoResult<()>;

    // === Multi-source values ===

    /// Set a field value with source attribution
    #[allow(clippy::too_many_arguments)] // Trait method with distinct semantic params
    fn set_value(
        &self,
        serial: &str,
        field: &str,
        value: &str,
        source: ValueSource,
        source_detail: Option<&str>,
        confidence: Confidence,
    ) -> RepoResult<()>;

    /// Get all values for a field across sources
    fn get_values(&self, serial: &str, field: &str) -> RepoResult<Vec<ItemValue>>;

    /// Get the best value for a field
    fn get_best_value(&self, serial: &str, field: &str) -> RepoResult<Option<ItemValue>>;

    /// Get all values for an item
    fn get_all_values(&self, serial: &str) -> RepoResult<Vec<ItemValue>>;

    /// Get best value for each field as a map
    fn get_best_values(&self, serial: &str) -> RepoResult<HashMap<String, String>>;

    /// Get best values for all items (bulk query)
    fn get_all_items_best_values(&self) -> RepoResult<HashMap<String, HashMap<String, String>>>;

    // === Statistics ===

    /// Get database statistics
    fn stats(&self) -> RepoResult<DbStats>;

    // === Migration ===

    /// Migrate column values to item_values table
    fn migrate_column_values(&self, dry_run: bool) -> RepoResult<MigrationStats>;
}

/// Extension trait for attachment operations (feature-gated)
#[cfg(feature = "attachments")]
pub trait AttachmentsRepository {
    /// Add an image attachment
    #[allow(clippy::too_many_arguments)] // Trait method with distinct semantic params
    fn add_attachment(
        &self,
        serial: &str,
        name: &str,
        mime_type: &str,
        data: &[u8],
        view: &str,
    ) -> RepoResult<i64>;

    /// Get attachments for an item (without data)
    fn get_attachments(&self, serial: &str) -> RepoResult<Vec<Attachment>>;

    /// Get attachment data by ID
    fn get_attachment_data(&self, id: i64) -> RepoResult<Option<Vec<u8>>>;

    /// Delete an attachment
    fn delete_attachment(&self, id: i64) -> RepoResult<bool>;
}

/// Extension trait for import/export operations
pub trait ImportExportRepository {
    /// Import an item from a directory
    fn import_from_dir(&self, dir: &std::path::Path) -> RepoResult<String>;

    /// Export an item to a directory
    fn export_to_dir(&self, serial: &str, dir: &std::path::Path) -> RepoResult<()>;
}

/// Extension trait for bulk operations
pub trait BulkRepository {
    /// Add multiple items at once
    fn add_items_bulk(&self, serials: &[&str]) -> RepoResult<BulkResult>;

    /// Set values for multiple items
    fn set_values_bulk(&self, values: &[BulkValueSet]) -> RepoResult<BulkResult>;
}

/// Request for bulk value setting
#[derive(Debug, Clone)]
pub struct BulkValueSet {
    pub serial: String,
    pub field: String,
    pub value: String,
    pub source: ValueSource,
    pub source_detail: Option<String>,
    pub confidence: Confidence,
}

/// Result of a bulk operation
#[derive(Debug, Clone, Default)]
pub struct BulkResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<(String, String)>, // (serial, error message)
}
