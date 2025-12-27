//! Type definitions for file-based extraction

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::super::property_parsing::AssetInfo;

/// Get manufacturer names from bl4::reference
pub fn manufacturer_names() -> HashMap<&'static str, &'static str> {
    bl4::reference::MANUFACTURERS
        .iter()
        .map(|m| (m.code, m.name))
        .collect()
}

/// Manufacturer found during directory walking (distinct from ExtractedManufacturer)
#[derive(Debug, Serialize, Deserialize)]
pub struct Manufacturer {
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_data_path: Option<String>,
}

/// Weapon type with associated manufacturers
#[derive(Debug, Serialize, Deserialize)]
pub struct WeaponType {
    pub name: String,
    pub path: String,
    pub manufacturers: Vec<ManufacturerRef>,
}

/// Reference to a manufacturer within a weapon/gear type
#[derive(Debug, Serialize, Deserialize)]
pub struct ManufacturerRef {
    pub code: String,
    pub name: String,
    pub path: String,
}

/// Category of balance data assets
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceCategory {
    pub name: String,
    pub path: String,
    pub assets: Vec<AssetInfo>,
}

/// Gear type (shields, grenades, gadgets, etc.) with associated data
#[derive(Debug, Serialize, Deserialize)]
pub struct GearType {
    pub name: String,
    pub path: String,
    pub balance_data: Vec<AssetInfo>,
    pub manufacturers: Vec<ManufacturerRef>,
}
