//! Weapon and Gear Type extraction from PAK files
//!
//! Extracts authoritative weapon type and gear type data from pak_manifest.json.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::manifest::PakManifest;

/// Extracted weapon type with manufacturer data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedWeaponType {
    /// Internal name from game paths (e.g., "AssaultRifles")
    pub internal_name: String,
    /// Short code (e.g., "AR", "PS", "SG")
    pub code: String,
    /// Manufacturers that make this weapon type
    pub manufacturers: Vec<String>,
    /// Example paths where this weapon type appears
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub example_paths: Vec<String>,
}

/// Extract weapon types from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers weapon types and their manufacturers from game paths:
/// - /Gear/Weapons/<WeaponType>/<ManufacturerCode>/
/// - /Gear/Gadgets/HeavyWeapons/<ManufacturerCode>/
pub fn extract_weapon_types_from_pak(
    pak_manifest_path: &Path,
) -> Result<HashMap<String, ExtractedWeaponType>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut weapon_types: HashMap<String, ExtractedWeaponType> = HashMap::new();

    // Pattern to find weapon type + manufacturer from paths
    // e.g., /Gear/Weapons/AssaultRifles/DAD/ or /Gear/Gadgets/HeavyWeapons/TOR/
    let weapon_path_pattern = Regex::new(r"/Gear/Weapons/([^_/][^/]*)/([A-Z]{3})/").unwrap();
    let heavy_weapon_pattern = Regex::new(r"/Gear/Gadgets/HeavyWeapons/([A-Z]{3})/").unwrap();

    // Known weapon type name to code mappings (derived from part names in game)
    let type_to_code: HashMap<&str, &str> = [
        ("AssaultRifles", "AR"),
        ("Pistols", "PS"),
        ("Shotguns", "SG"),
        ("SMG", "SM"),
        ("Sniper", "SR"),
        ("HeavyWeapons", "HW"),
    ]
    .iter()
    .cloned()
    .collect();

    for item in &manifest.items {
        let path = &item.path;

        // Check for regular weapons
        if let Some(cap) = weapon_path_pattern.captures(path) {
            let weapon_type = cap[1].to_string();
            let mfr_code = cap[2].to_string();

            // Skip internal directories
            if weapon_type.starts_with('_')
                || weapon_type == "Materials"
                || weapon_type == "Textures"
                || weapon_type == "Systems"
                || weapon_type == "Uniques"
            {
                continue;
            }

            let code = type_to_code
                .get(weapon_type.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    weapon_type
                        .chars()
                        .take(2)
                        .collect::<String>()
                        .to_uppercase()
                });

            let wt =
                weapon_types
                    .entry(weapon_type.clone())
                    .or_insert_with(|| ExtractedWeaponType {
                        internal_name: weapon_type.clone(),
                        code,
                        manufacturers: Vec::new(),
                        example_paths: Vec::new(),
                    });

            if !wt.manufacturers.contains(&mfr_code) {
                wt.manufacturers.push(mfr_code);
            }

            if wt.example_paths.len() < 3 {
                wt.example_paths.push(path.clone());
            }
        }

        // Check for heavy weapons (under Gadgets)
        if let Some(cap) = heavy_weapon_pattern.captures(path) {
            let mfr_code = cap[1].to_string();

            let wt = weapon_types
                .entry("HeavyWeapons".to_string())
                .or_insert_with(|| ExtractedWeaponType {
                    internal_name: "HeavyWeapons".to_string(),
                    code: "HW".to_string(),
                    manufacturers: Vec::new(),
                    example_paths: Vec::new(),
                });

            if !wt.manufacturers.contains(&mfr_code) {
                wt.manufacturers.push(mfr_code);
            }

            if wt.example_paths.len() < 3 {
                wt.example_paths.push(path.clone());
            }
        }
    }

    // Sort manufacturers within each weapon type
    for wt in weapon_types.values_mut() {
        wt.manufacturers.sort();
    }

    Ok(weapon_types)
}

/// Extracted gear type (non-weapon equipment)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedGearType {
    /// Internal name from game paths (e.g., "Shields", "GrenadeGadgets")
    pub internal_name: String,
    /// Manufacturers that make this gear type
    pub manufacturers: Vec<String>,
    /// Subcategories if any
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subcategories: Vec<String>,
    /// Example paths where this gear type appears
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub example_paths: Vec<String>,
}

/// Extract gear types from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers gear types and their manufacturers from game paths:
/// - /Gear/<GearType>/Manufacturer/<ManufacturerCode>/
/// - /Gear/Gadgets/<Subcategory>/<ManufacturerCode>/
pub fn extract_gear_types_from_pak(
    pak_manifest_path: &Path,
) -> Result<HashMap<String, ExtractedGearType>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut gear_types: HashMap<String, ExtractedGearType> = HashMap::new();

    // Pattern for gear types with manufacturer subdirs
    // e.g., /Gear/Shields/Manufacturer/BOR/
    let gear_mfr_pattern = Regex::new(r"/Gear/([^_/][^/]*)/Manufacturer/([A-Z]{3})/").unwrap();

    // Pattern for gear types without explicit Manufacturer dir
    // e.g., /Gear/Gadgets/Turrets/DPL/
    let gadget_pattern = Regex::new(r"/Gear/Gadgets/([^_/][^/]*)/([A-Z]{3})/").unwrap();

    // Pattern for general gear paths to find subcategories
    let gear_path_pattern = Regex::new(r"/Gear/([^_/][^/]*)/").unwrap();

    for item in &manifest.items {
        let path = &item.path;

        // Check for gear with manufacturer subdirectories
        if let Some(cap) = gear_mfr_pattern.captures(path) {
            let gear_type = cap[1].to_string();
            let mfr_code = cap[2].to_string();

            // Normalize shields casing
            let gear_type_normalized = if gear_type.to_lowercase() == "shields" {
                "Shields".to_string()
            } else {
                gear_type
            };

            let gt = gear_types
                .entry(gear_type_normalized.clone())
                .or_insert_with(|| ExtractedGearType {
                    internal_name: gear_type_normalized.clone(),
                    manufacturers: Vec::new(),
                    subcategories: Vec::new(),
                    example_paths: Vec::new(),
                });

            if !gt.manufacturers.contains(&mfr_code) {
                gt.manufacturers.push(mfr_code);
            }

            if gt.example_paths.len() < 3 {
                gt.example_paths.push(path.clone());
            }
        }

        // Check for gadgets subcategories
        if let Some(cap) = gadget_pattern.captures(path) {
            let subcategory = cap[1].to_string();
            let mfr_code = cap[2].to_string();

            // Skip HeavyWeapons as they're handled in weapon types
            if subcategory == "HeavyWeapons" {
                continue;
            }

            let gt = gear_types
                .entry("Gadgets".to_string())
                .or_insert_with(|| ExtractedGearType {
                    internal_name: "Gadgets".to_string(),
                    manufacturers: Vec::new(),
                    subcategories: Vec::new(),
                    example_paths: Vec::new(),
                });

            if !gt.subcategories.contains(&subcategory) {
                gt.subcategories.push(subcategory);
            }

            if !gt.manufacturers.contains(&mfr_code) {
                gt.manufacturers.push(mfr_code);
            }

            if gt.example_paths.len() < 3 {
                gt.example_paths.push(path.clone());
            }
        }

        // Find other gear types without manufacturers
        if let Some(cap) = gear_path_pattern.captures(path) {
            let gear_type = cap[1].to_string();

            // Skip already handled types and internal directories
            if gear_type.starts_with('_')
                || gear_type == "Weapons"
                || gear_type.to_lowercase() == "shields"
                || gear_type == "GrenadeGadgets"
                || gear_type == "Gadgets"
                || gear_type == "Effects"
            {
                continue;
            }

            let gt = gear_types
                .entry(gear_type.clone())
                .or_insert_with(|| ExtractedGearType {
                    internal_name: gear_type.clone(),
                    manufacturers: Vec::new(),
                    subcategories: Vec::new(),
                    example_paths: Vec::new(),
                });

            if gt.example_paths.len() < 3 && !gt.example_paths.contains(&path.clone()) {
                gt.example_paths.push(path.clone());
            }
        }
    }

    // Sort manufacturers within each gear type
    for gt in gear_types.values_mut() {
        gt.manufacturers.sort();
        gt.subcategories.sort();
    }

    Ok(gear_types)
}
