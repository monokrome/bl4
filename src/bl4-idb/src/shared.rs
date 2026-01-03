//! Shared constants and query building utilities for database implementations.
//!
//! This module contains SQL constants, column definitions, and query building
//! logic that is shared between the synchronous (rusqlite) and asynchronous
//! (sqlx) database implementations.

use crate::types::ItemFilter;

/// Column names for the items table, used in SELECT queries.
/// Order must match the positional indices used in row mapping functions.
pub const ITEM_COLUMNS: &[&str] = &[
    "serial",
    "name",
    "prefix",
    "manufacturer",
    "weapon_type",
    "item_type",
    "rarity",
    "level",
    "element",
    "dps",
    "damage",
    "accuracy",
    "fire_rate",
    "reload_time",
    "mag_size",
    "value",
    "red_text",
    "notes",
    "verification_status",
    "verification_notes",
    "verified_at",
    "legal",
    "source",
    "created_at",
];

/// Comma-separated column list for SELECT queries
pub const ITEM_SELECT_COLUMNS: &str = "serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                    dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                    notes, verification_status, verification_notes, verified_at, legal, source, created_at";

/// Column names for item_values table
pub const ITEM_VALUE_COLUMNS: &str =
    "id, item_serial, field, value, source, source_detail, confidence, created_at";

/// Fields that can be migrated from item columns to item_values table
pub const FIELDS_TO_MIGRATE: &[(&str, &str)] = &[
    ("name", "name"),
    ("prefix", "prefix"),
    ("manufacturer", "manufacturer"),
    ("weapon_type", "weapon_type"),
    ("item_type", "item_type"),
    ("rarity", "rarity"),
    ("level", "level"),
    ("element", "element"),
    ("dps", "dps"),
    ("damage", "damage"),
    ("accuracy", "accuracy"),
    ("fire_rate", "fire_rate"),
    ("reload_time", "reload_time"),
    ("mag_size", "mag_size"),
    ("value", "value"),
    ("red_text", "red_text"),
];

/// SQLite schema definitions
pub mod schema {
    /// Items table schema (SQLite)
    pub const ITEMS_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS items (
            serial TEXT PRIMARY KEY NOT NULL,
            name TEXT,
            prefix TEXT,
            manufacturer TEXT,
            weapon_type TEXT,
            item_type TEXT,
            rarity TEXT,
            level INTEGER,
            element TEXT,
            dps INTEGER,
            damage INTEGER,
            accuracy INTEGER,
            fire_rate REAL,
            reload_time REAL,
            mag_size INTEGER,
            value INTEGER,
            red_text TEXT,
            notes TEXT,
            verification_status TEXT DEFAULT 'unverified',
            verification_notes TEXT,
            verified_at TIMESTAMP,
            legal BOOLEAN DEFAULT FALSE,
            source TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    "#;

    /// Item parts table schema (SQLite)
    pub const ITEM_PARTS_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS item_parts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_serial TEXT NOT NULL REFERENCES items(serial) ON DELETE CASCADE,
            slot TEXT NOT NULL,
            part_index INTEGER,
            part_name TEXT,
            manufacturer TEXT,
            effect TEXT,
            verified BOOLEAN DEFAULT FALSE,
            verification_method TEXT,
            verification_notes TEXT,
            verified_at TIMESTAMP
        )
    "#;

    /// Attachments table schema (SQLite)
    pub const ATTACHMENTS_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_serial TEXT NOT NULL REFERENCES items(serial) ON DELETE CASCADE,
            name TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            data BLOB NOT NULL,
            view TEXT DEFAULT 'OTHER'
        )
    "#;

    /// Item values table schema (SQLite)
    pub const ITEM_VALUES_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS item_values (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_serial TEXT NOT NULL REFERENCES items(serial) ON DELETE CASCADE,
            field TEXT NOT NULL,
            value TEXT NOT NULL,
            source TEXT NOT NULL,
            source_detail TEXT,
            confidence TEXT NOT NULL DEFAULT 'inferred',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(item_serial, field, source)
        )
    "#;

    /// Settings table schema
    pub const SETTINGS_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        )
    "#;

    /// Schema migrations tracking table
    pub const MIGRATIONS_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY NOT NULL,
            applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    "#;

    /// Index definitions for SQLite (use after table creation)
    pub const INDEXES: &[&str] = &[
        "CREATE INDEX IF NOT EXISTS idx_items_name ON items(name)",
        "CREATE INDEX IF NOT EXISTS idx_items_manufacturer ON items(manufacturer)",
        "CREATE INDEX IF NOT EXISTS idx_item_parts_item_serial ON item_parts(item_serial)",
        "CREATE INDEX IF NOT EXISTS idx_item_values_serial ON item_values(item_serial)",
        "CREATE INDEX IF NOT EXISTS idx_item_values_field ON item_values(item_serial, field)",
        "CREATE INDEX IF NOT EXISTS idx_attachments_item_serial ON attachments(item_serial)",
    ];
}

/// Build a list query with optional filters.
///
/// Returns the SQL string with placeholders. The caller is responsible for
/// binding parameters in the order they were added to the filter.
///
/// # Arguments
/// * `filter` - The filter to apply
/// * `placeholder_style` - Either `?` for SQLite/MySQL or `$1, $2, ...` for PostgreSQL
///
/// # Returns
/// A tuple of (sql_string, param_count) where param_count indicates how many
/// parameters need to be bound.
pub fn build_list_query(filter: &ItemFilter, use_dollar_placeholders: bool) -> (String, usize) {
    let mut sql = format!(
        "SELECT {} FROM items WHERE 1=1",
        ITEM_SELECT_COLUMNS
    );
    let mut param_count = 0;

    fn next_placeholder(use_dollar: bool, count: &mut usize) -> String {
        *count += 1;
        if use_dollar {
            format!("${}", *count)
        } else {
            "?".to_string()
        }
    }

    if filter.manufacturer.is_some() {
        sql.push_str(&format!(
            " AND manufacturer = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }
    if filter.weapon_type.is_some() {
        sql.push_str(&format!(
            " AND weapon_type = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }
    if filter.element.is_some() {
        sql.push_str(&format!(
            " AND element = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }
    if filter.rarity.is_some() {
        sql.push_str(&format!(
            " AND rarity = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }

    sql.push_str(" ORDER BY created_at DESC");

    if let Some(limit) = filter.limit {
        sql.push_str(&format!(" LIMIT {}", limit));
    }
    if let Some(offset) = filter.offset {
        sql.push_str(&format!(" OFFSET {}", offset));
    }

    (sql, param_count)
}

/// Build a count query with optional filters.
///
/// Returns the SQL string with placeholders. The caller is responsible for
/// binding parameters in the order they were added to the filter.
pub fn build_count_query(filter: &ItemFilter, use_dollar_placeholders: bool) -> (String, usize) {
    let mut sql = String::from("SELECT COUNT(*) as count FROM items WHERE 1=1");
    let mut param_count = 0;

    fn next_placeholder(use_dollar: bool, count: &mut usize) -> String {
        *count += 1;
        if use_dollar {
            format!("${}", *count)
        } else {
            "?".to_string()
        }
    }

    if filter.manufacturer.is_some() {
        sql.push_str(&format!(
            " AND manufacturer = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }
    if filter.weapon_type.is_some() {
        sql.push_str(&format!(
            " AND weapon_type = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }
    if filter.element.is_some() {
        sql.push_str(&format!(
            " AND element = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }
    if filter.rarity.is_some() {
        sql.push_str(&format!(
            " AND rarity = {}",
            next_placeholder(use_dollar_placeholders, &mut param_count)
        ));
    }

    (sql, param_count)
}

/// Common SQL queries used across implementations
pub mod queries {
    /// Insert a new item
    pub const INSERT_ITEM: &str = "INSERT INTO items (serial) VALUES (?)";
    pub const INSERT_ITEM_PG: &str = "INSERT INTO items (serial) VALUES ($1)";

    /// Get item by serial
    pub const GET_ITEM: &str = "SELECT {} FROM items WHERE serial = ?";
    pub const GET_ITEM_PG: &str = "SELECT {} FROM items WHERE serial = $1";

    /// Delete item
    pub const DELETE_ITEM: &str = "DELETE FROM items WHERE serial = ?";
    pub const DELETE_ITEM_PG: &str = "DELETE FROM items WHERE serial = $1";

    /// Update verification status
    pub const UPDATE_VERIFICATION: &str = r#"UPDATE items SET
        verification_status = ?,
        verification_notes = COALESCE(?, verification_notes),
        verified_at = CASE WHEN ? != 'unverified' THEN CURRENT_TIMESTAMP ELSE verified_at END
    WHERE serial = ?"#;

    /// Set legal status
    pub const SET_LEGAL: &str = "UPDATE items SET legal = ? WHERE serial = ?";
    pub const SET_LEGAL_PG: &str = "UPDATE items SET legal = $1 WHERE serial = $2";

    /// Set item type
    pub const SET_ITEM_TYPE: &str = "UPDATE items SET item_type = ? WHERE serial = ?";
    pub const SET_ITEM_TYPE_PG: &str = "UPDATE items SET item_type = $1 WHERE serial = $2";

    /// Set source
    pub const SET_SOURCE: &str = "UPDATE items SET source = ? WHERE serial = ?";
    pub const SET_SOURCE_PG: &str = "UPDATE items SET source = $1 WHERE serial = $2";

    /// Upsert item value
    pub const UPSERT_VALUE: &str = r#"INSERT OR REPLACE INTO item_values
        (item_serial, field, value, source, source_detail, confidence)
        VALUES (?, ?, ?, ?, ?, ?)"#;

    /// Get values for a field
    pub const GET_VALUES: &str = r#"SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
        FROM item_values
        WHERE item_serial = ? AND field = ?
        ORDER BY source DESC, confidence DESC"#;

    /// Get all values for an item
    pub const GET_ALL_VALUES: &str = r#"SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
        FROM item_values
        WHERE item_serial = ?
        ORDER BY field, source DESC, confidence DESC"#;

    /// Get setting
    pub const GET_SETTING: &str = "SELECT value FROM settings WHERE key = ?";
    pub const GET_SETTING_PG: &str = "SELECT value FROM settings WHERE key = $1";

    /// Set setting (SQLite)
    pub const SET_SETTING: &str =
        "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value";
    pub const SET_SETTING_PG: &str =
        "INSERT INTO settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2";

    /// Get distinct sources
    pub const GET_DISTINCT_SOURCES: &str =
        "SELECT DISTINCT source FROM items WHERE source IS NOT NULL";

    /// Stats queries
    pub const COUNT_ITEMS: &str = "SELECT COUNT(*) FROM items";
    pub const COUNT_PARTS: &str = "SELECT COUNT(*) FROM item_parts";
    pub const COUNT_ATTACHMENTS: &str = "SELECT COUNT(*) FROM attachments";
    pub const COUNT_VALUES: &str = "SELECT COUNT(*) FROM item_values";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_list_query_no_filters() {
        let filter = ItemFilter::default();
        let (sql, count) = build_list_query(&filter, false);

        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM items"));
        assert!(sql.contains("ORDER BY created_at DESC"));
        assert_eq!(count, 0);
    }

    #[test]
    fn test_build_list_query_with_manufacturer() {
        let filter = ItemFilter {
            manufacturer: Some("DAD".to_string()),
            ..Default::default()
        };
        let (sql, count) = build_list_query(&filter, false);

        assert!(sql.contains("AND manufacturer = ?"));
        assert_eq!(count, 1);
    }

    #[test]
    fn test_build_list_query_with_all_filters() {
        let filter = ItemFilter {
            manufacturer: Some("DAD".to_string()),
            weapon_type: Some("Pistol".to_string()),
            element: Some("Fire".to_string()),
            rarity: Some("Legendary".to_string()),
            limit: Some(10),
            offset: Some(5),
        };
        let (sql, count) = build_list_query(&filter, false);

        assert!(sql.contains("AND manufacturer = ?"));
        assert!(sql.contains("AND weapon_type = ?"));
        assert!(sql.contains("AND element = ?"));
        assert!(sql.contains("AND rarity = ?"));
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("OFFSET 5"));
        assert_eq!(count, 4);
    }

    #[test]
    fn test_build_list_query_postgres_placeholders() {
        let filter = ItemFilter {
            manufacturer: Some("DAD".to_string()),
            rarity: Some("Legendary".to_string()),
            ..Default::default()
        };
        let (sql, count) = build_list_query(&filter, true);

        assert!(sql.contains("manufacturer = $1"));
        assert!(sql.contains("rarity = $2"));
        assert_eq!(count, 2);
    }

    #[test]
    fn test_build_count_query() {
        let filter = ItemFilter {
            manufacturer: Some("DAD".to_string()),
            ..Default::default()
        };
        let (sql, count) = build_count_query(&filter, false);

        assert!(sql.contains("SELECT COUNT(*)"));
        assert!(sql.contains("AND manufacturer = ?"));
        assert_eq!(count, 1);
    }

    #[test]
    fn test_item_columns_count() {
        assert_eq!(ITEM_COLUMNS.len(), 24);
    }

    #[test]
    fn test_fields_to_migrate_count() {
        assert_eq!(FIELDS_TO_MIGRATE.len(), 16);
    }
}
