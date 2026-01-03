//! Manufacturer extraction from PAK files
//!
//! Extracts authoritative manufacturer data from pak_manifest.json.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::manifest::PakManifest;

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
    let manifest = PakManifest::load(pak_manifest_path)?;

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
