//! PAK file extraction functions for game data
//!
//! Extracts authoritative game data from pak_manifest.json including:
//! - Manufacturers and their codes
//! - Weapon types and their manufacturers
//! - Gear types (shields, gadgets, etc.)
//! - Element types
//! - Rarity tiers
//! - Stat types and modifiers

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::PakManifest;

/// Get manufacturer names from bl4::reference
/// DEPRECATED: Use `extract_manufacturer_names_from_pak` for authoritative data.
#[deprecated(
    since = "0.5.0",
    note = "Use extract_manufacturer_names_from_pak for authoritative game data"
)]
pub fn manufacturer_names() -> HashMap<&'static str, &'static str> {
    bl4::reference::MANUFACTURERS
        .iter()
        .map(|m| (m.code, m.name))
        .collect()
}

/// Extracted manufacturer with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedManufacturer {
    /// 3-letter code (e.g., "TOR")
    pub code: String,
    /// Full name extracted from game (e.g., "Torgue")
    pub name: String,
    /// How the name was discovered
    pub name_source: String,
    /// Game paths where this manufacturer appears
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

/// Extract manufacturer names from pak_manifest.json (AUTHORITATIVE)
///
/// This extracts manufacturer code→name mappings from actual game data by:
/// 1. WeaponAnimation paths: /WeaponAnimation/<type>/<FullName>/..._<CODE>_...
/// 2. _Manufacturer paths: /_Manufacturer/<CODE>/...<FullName>...
/// 3. UI logo paths: ui_art_manu_logomark_<fullname>
///
/// Priority is given to high-confidence sources (animation/manufacturer paths).
pub fn extract_manufacturer_names_from_pak(
    pak_manifest_path: &Path,
) -> Result<HashMap<String, ExtractedManufacturer>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut manufacturers: HashMap<String, ExtractedManufacturer> = HashMap::new();

    // Pattern to find manufacturer codes in filenames (e.g., ATrick_TOR_AR, NS_SG_BOR)
    let code_in_filename = Regex::new(r"[/_]([A-Z]{3})_[A-Z]{2}[_.]").unwrap();

    // Pattern to find WeaponAnimation/<type>/<ManufacturerName>/ structure
    let weapon_anim_pattern = Regex::new(r"WeaponAnimation/[^/]+/([A-Za-z]+)/").unwrap();

    // Pattern to find _Manufacturer/<CODE>/ paths
    let manufacturer_dir_pattern = Regex::new(r"_Manufacturer/([A-Z]{3})/").unwrap();

    // Pattern for UI logo paths
    let ui_logo_pattern = Regex::new(
        r"ui_art_manu_(?:logomark|logotype|itemcard_logomark|itemcard_logotype)_([a-z]+)",
    )
    .unwrap();

    // Known 3-letter codes that appear in manufacturer paths
    let potential_codes: std::collections::HashSet<&str> = [
        "BOR", "DAD", "DPL", "JAK", "MAL", "ORD", "RIP", "TED", "TOR", "VLA", "COV", "GRV",
    ]
    .iter()
    .copied()
    .collect();

    // First pass: discover code→name mappings from reliable sources
    let mut code_to_name: HashMap<String, (String, String, u8)> = HashMap::new(); // (name, source, priority)

    for item in &manifest.items {
        let path = &item.path;
        let path_lower = path.to_lowercase();

        // HIGH PRIORITY: WeaponAnimation paths with full name in directory
        // e.g., /WeaponAnimation/AssaultRifle/Torgue/AS_AR_TOR_Mode.uasset
        if let Some(anim_cap) = weapon_anim_pattern.captures(path) {
            let folder_name = anim_cap[1].to_string();
            // Check if there's a code in the filename
            if let Some(code_cap) = code_in_filename.captures(path) {
                let code = code_cap[1].to_string();
                if potential_codes.contains(code.as_str()) {
                    let existing_priority =
                        code_to_name.get(&code).map(|(_, _, p)| *p).unwrap_or(0);
                    if existing_priority < 10 {
                        code_to_name.insert(
                            code.clone(),
                            (
                                folder_name.clone(),
                                format!("WeaponAnimation folder: {}", path),
                                10,
                            ),
                        );
                    }
                }
            }
        }

        // HIGH PRIORITY: _Manufacturer paths with full name in asset
        // e.g., /_Manufacturer/TOR/Script_Weapon_TorgueSticky.uasset
        if let Some(mfr_cap) = manufacturer_dir_pattern.captures(path) {
            let code = mfr_cap[1].to_string();
            if potential_codes.contains(code.as_str()) {
                // Look for manufacturer name in the filename
                let filename = path.split('/').next_back().unwrap_or("");
                let filename_lower = filename.to_lowercase();

                let candidate_names = [
                    ("borg", "Borg"),
                    ("daedalus", "Daedalus"),
                    ("dahl", "Dahl"),
                    ("jakobs", "Jakobs"),
                    ("maliwan", "Maliwan"),
                    ("order", "Order"),
                    ("ripper", "Ripper"),
                    ("tediore", "Tediore"),
                    ("torgue", "Torgue"),
                    ("vladof", "Vladof"),
                    ("gravitar", "Gravitar"),
                ];

                for (name_lower, name_title) in candidate_names {
                    if filename_lower.contains(name_lower) {
                        let existing_priority =
                            code_to_name.get(&code).map(|(_, _, p)| *p).unwrap_or(0);
                        if existing_priority < 9 {
                            code_to_name.insert(
                                code.clone(),
                                (
                                    name_title.to_string(),
                                    format!("_Manufacturer path: {}", path),
                                    9,
                                ),
                            );
                        }
                        break;
                    }
                }
            }
        }

        // MEDIUM PRIORITY: UI logo paths (reliable but only for names, not code mapping)
        if path_lower.contains("ui_art_manu") {
            if let Some(cap) = ui_logo_pattern.captures(&path_lower) {
                let name = cap[1].to_string();
                let name_title = name
                    .chars()
                    .enumerate()
                    .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                    .collect::<String>();

                // Store with UI_ prefix - will try to match to codes later
                code_to_name
                    .entry(format!("UI_{}", name.to_uppercase()))
                    .or_insert((name_title, format!("UI logo: {}", path), 5));
            }
        }
    }

    // Try to match UI_ entries to actual codes by name similarity
    let ui_to_code: Vec<(String, String)> = vec![
        ("UI_TORGUE".to_string(), "TOR".to_string()),
        ("UI_VLADOF".to_string(), "VLA".to_string()),
        ("UI_JAKOBS".to_string(), "JAK".to_string()),
        ("UI_MALIWAN".to_string(), "MAL".to_string()),
        ("UI_TEDIORE".to_string(), "TED".to_string()),
        ("UI_DAEDALUS".to_string(), "DAD".to_string()),
        ("UI_ORDER".to_string(), "ORD".to_string()),
        ("UI_RIPPER".to_string(), "RIP".to_string()),
        ("UI_COV".to_string(), "COV".to_string()),
        ("UI_BORG".to_string(), "BOR".to_string()),
    ];

    for (ui_key, code) in ui_to_code {
        if let Some((name, source, priority)) = code_to_name.get(&ui_key) {
            let existing_priority = code_to_name.get(&code).map(|(_, _, p)| *p).unwrap_or(0);
            if existing_priority < *priority {
                code_to_name.insert(code.clone(), (name.clone(), source.clone(), *priority));
            }
        }
    }

    // Second pass: build final manufacturer list from all paths with codes
    let code_pattern = Regex::new(r"/([A-Z]{3})/").unwrap();

    for item in &manifest.items {
        for cap in code_pattern.captures_iter(&item.path) {
            let code = cap[1].to_string();

            if !potential_codes.contains(code.as_str()) {
                continue;
            }

            let mfr = manufacturers.entry(code.clone()).or_insert_with(|| {
                let (name, source, _) = code_to_name.get(&code).cloned().unwrap_or_else(|| {
                    (
                        code.clone(),
                        "Code only (full name not discovered)".to_string(),
                        0,
                    )
                });
                ExtractedManufacturer {
                    code: code.clone(),
                    name,
                    name_source: source,
                    paths: Vec::new(),
                }
            });

            // Add unique paths (limit to 5)
            if !mfr.paths.contains(&item.path) && mfr.paths.len() < 5 {
                mfr.paths.push(item.path.clone());
            }
        }
    }

    // Also add manufacturers from the manifest's own list
    for code in &manifest.manufacturers {
        if !manufacturers.contains_key(code) {
            let (name, source, _) = code_to_name.get(code).cloned().unwrap_or_else(|| {
                (
                    code.clone(),
                    "Code only (full name not discovered)".to_string(),
                    0,
                )
            });
            manufacturers.insert(
                code.clone(),
                ExtractedManufacturer {
                    code: code.clone(),
                    name,
                    name_source: source,
                    paths: Vec::new(),
                },
            );
        }
    }

    Ok(manufacturers)
}

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

/// Extracted element type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedElement {
    /// Internal name from game paths (e.g., "Fire", "Cryo", "Shock")
    pub internal_name: String,
    /// Example paths where this element appears
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub example_paths: Vec<String>,
}

/// Extract element types from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers element types from game effect/texture paths:
/// - /Common/Effects/Textures/Elements/<ElementType>/
/// - /Common/Effects/Materials/Elements/<ElementType>/
pub fn extract_elements_from_pak(
    pak_manifest_path: &Path,
) -> Result<HashMap<String, ExtractedElement>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut elements: HashMap<String, ExtractedElement> = HashMap::new();

    // Pattern to find element types from effect paths
    // e.g., /Common/Effects/Textures/Elements/Fire/
    let element_pattern =
        Regex::new(r"/(?:Effects|Materials)/(?:Textures|Materials)?/?Elements/([A-Za-z]+)/")
            .unwrap();

    for item in &manifest.items {
        let path = &item.path;

        if let Some(cap) = element_pattern.captures(path) {
            let element_name = cap[1].to_string();

            // Skip partial matches (like "M" or "T" from file prefixes)
            if element_name.len() < 3 {
                continue;
            }

            let elem = elements
                .entry(element_name.clone())
                .or_insert_with(|| ExtractedElement {
                    internal_name: element_name.clone(),
                    example_paths: Vec::new(),
                });

            if elem.example_paths.len() < 3 {
                elem.example_paths.push(path.clone());
            }
        }
    }

    Ok(elements)
}

/// Extracted rarity tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRarity {
    /// Tier number (1-5)
    pub tier: u8,
    /// Internal code (e.g., "comp_01")
    pub code: String,
    /// Name from game (e.g., "common", "legendary")
    pub name: String,
    /// Example paths where this rarity appears
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub example_paths: Vec<String>,
}

/// Extract rarity tiers from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers rarity tiers from UI rarity pip assets:
/// - rarity_pip_01_common, rarity_pip_02_uncommon, etc.
pub fn extract_rarities_from_pak(pak_manifest_path: &Path) -> Result<Vec<ExtractedRarity>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut rarities: HashMap<u8, ExtractedRarity> = HashMap::new();

    // Pattern to find rarity tiers from UI pip assets
    // e.g., rarity_pip_01_common, rarity_pip_05_legendary
    let rarity_pip_pattern = Regex::new(r"rarity_pip_(\d{2})_([a-z]+)").unwrap();

    // Also look for comp_XX patterns in part names
    let comp_pattern = Regex::new(r"comp_(\d{2})_([a-z]+)").unwrap();

    for item in &manifest.items {
        let path = &item.path;
        let path_lower = path.to_lowercase();

        // Check for rarity pip assets
        if let Some(cap) = rarity_pip_pattern.captures(&path_lower) {
            let tier: u8 = cap[1].parse().unwrap_or(0);
            let name = cap[2].to_string();

            if (1..=5).contains(&tier) {
                let rarity = rarities.entry(tier).or_insert_with(|| ExtractedRarity {
                    tier,
                    code: format!("comp_{:02}", tier),
                    name: name.clone(),
                    example_paths: Vec::new(),
                });

                if rarity.example_paths.len() < 3 {
                    rarity.example_paths.push(path.clone());
                }
            }
        }

        // Also check property names for comp_XX patterns
        for prop in &item.property_names {
            let prop_lower = prop.to_lowercase();
            if let Some(cap) = comp_pattern.captures(&prop_lower) {
                let tier: u8 = cap[1].parse().unwrap_or(0);
                let name = cap[2].to_string();

                if (1..=5).contains(&tier) {
                    let rarity = rarities.entry(tier).or_insert_with(|| ExtractedRarity {
                        tier,
                        code: format!("comp_{:02}", tier),
                        name: name.clone(),
                        example_paths: Vec::new(),
                    });

                    // Update name if we find it from parts (more reliable than UI)
                    if rarity.name.is_empty() || rarity.name == "unknown" {
                        rarity.name = name;
                    }
                }
            }
        }
    }

    // Convert to sorted vector
    let mut result: Vec<ExtractedRarity> = rarities.into_values().collect();
    result.sort_by_key(|r| r.tier);

    Ok(result)
}

/// Extracted stat type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedStat {
    /// Stat name (e.g., "Damage", "Accuracy", "Spread")
    pub name: String,
    /// Modifier types found for this stat (Scale, Value, Add, Percent)
    pub modifier_types: Vec<String>,
    /// Number of occurrences in game data
    pub occurrences: usize,
}

/// Extract stat types from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers stat types from property names in game assets:
/// - Pattern: StatName_ModifierType_Index_GUID
/// - e.g., Damage_Scale_44_..., Accuracy_Value_38_...
pub fn extract_stats_from_pak(pak_manifest_path: &Path) -> Result<Vec<ExtractedStat>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut stats: HashMap<String, (std::collections::HashSet<String>, usize)> = HashMap::new();

    // Pattern to find stat properties
    // e.g., Damage_Scale_44_GUID, Accuracy_Value_38_GUID
    let stat_pattern =
        Regex::new(r"^([A-Z][a-zA-Z]+)_(Scale|Add|Value|Percent)_\d+_[A-F0-9]{32}$").unwrap();

    // Also simpler patterns like StatName_Index_GUID
    let simple_stat_pattern = Regex::new(r"^([A-Z][a-zA-Z]+)_\d+_[A-F0-9]{32}$").unwrap();

    // Known stat-like property prefixes to look for
    let stat_prefixes = [
        "Accuracy",
        "Damage",
        "CritDamage",
        "FireRate",
        "ReloadTime",
        "ReloadSpeed",
        "MagSize",
        "Spread",
        "Recoil",
        "Sway",
        "Ammo",
        "AmmoCost",
        "Capacity",
        "Cooldown",
        "Duration",
        "Healing",
        "Health",
        "Impulse",
        "Projectile",
        "Radius",
        "Regen",
        "Speed",
        "StatusChance",
        "StatusDamage",
        "ElementalPower",
        "DamageRadius",
        "EquipTime",
        "PutDownTime",
        "ZoomDuration",
        "AccImpulse",
        "AccRegen",
        "AccDelay",
        "ProjectilesPerShot",
    ];

    for item in &manifest.items {
        for prop in &item.property_names {
            // Check for StatName_ModifierType_Index_GUID pattern
            if let Some(cap) = stat_pattern.captures(prop) {
                let stat_name = cap[1].to_string();
                let modifier_type = cap[2].to_string();

                let entry = stats
                    .entry(stat_name)
                    .or_insert_with(|| (std::collections::HashSet::new(), 0));
                entry.0.insert(modifier_type);
                entry.1 += 1;
            }

            // Also check simple pattern for known stats
            if let Some(cap) = simple_stat_pattern.captures(prop) {
                let stat_name = cap[1].to_string();
                if stat_prefixes.contains(&stat_name.as_str()) {
                    let entry = stats
                        .entry(stat_name)
                        .or_insert_with(|| (std::collections::HashSet::new(), 0));
                    entry.1 += 1;
                }
            }
        }
    }

    // Convert to sorted vector
    let mut result: Vec<ExtractedStat> = stats
        .into_iter()
        .map(|(name, (modifiers, count))| {
            let mut modifier_types: Vec<String> = modifiers.into_iter().collect();
            modifier_types.sort();
            ExtractedStat {
                name,
                modifier_types,
                occurrences: count,
            }
        })
        .collect();

    result.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));

    Ok(result)
}
