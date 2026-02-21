//! Data table type definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single row in a data table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTableRow {
    /// Row identifier (e.g., "WeaponDamageScale", "Pistol", "Badass")
    pub row_name: String,
    /// Field name → value mapping (GUID suffixes stripped from keys)
    pub fields: HashMap<String, String>,
}

/// A parsed UE data table definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTable {
    /// Internal entry key (lowercase, e.g., "weapon_elementaldamagescale")
    pub key: String,
    /// Display name from gbx_ue_data_table field (e.g., "Weapon_ElementalDamageScale")
    pub name: String,
    /// UE5 asset path for the row struct schema
    pub row_struct: String,
    /// Parsed rows
    pub rows: Vec<DataTableRow>,
}

/// Collection of all data tables from a gbx_ue_data_table NCS file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTableManifest {
    /// Table key → DataTable
    pub tables: HashMap<String, DataTable>,
}

impl DataTableManifest {
    /// Get a table by key (case-insensitive)
    pub fn get(&self, key: &str) -> Option<&DataTable> {
        let lower = key.to_lowercase();
        self.tables.get(&lower)
    }

    /// Get all table keys, sorted
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.tables.keys().map(|s| s.as_str()).collect();
        keys.sort();
        keys
    }

    /// Total number of tables
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Whether the manifest is empty
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// Total number of rows across all tables
    pub fn total_rows(&self) -> usize {
        self.tables.values().map(|t| t.rows.len()).sum()
    }
}
