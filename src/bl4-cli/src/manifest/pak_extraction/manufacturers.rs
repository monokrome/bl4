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

fn discover_weapon_animation_name(
    path: &str,
    code_to_name: &mut HashMap<String, (String, String, u8)>,
    weapon_anim_pattern: &Regex,
    code_in_filename: &Regex,
    potential_codes: &std::collections::HashSet<&str>,
) {
    let Some(anim_cap) = weapon_anim_pattern.captures(path) else {
        return;
    };
    let Some(code_cap) = code_in_filename.captures(path) else {
        return;
    };

    let folder_name = anim_cap[1].to_string();
    let code = code_cap[1].to_string();
    if potential_codes.contains(code.as_str()) {
        let existing = code_to_name.get(&code).map(|(_, _, p)| *p).unwrap_or(0);
        if existing < 10 {
            code_to_name.insert(
                code,
                (folder_name, format!("WeaponAnimation folder: {}", path), 10),
            );
        }
    }
}

fn discover_manufacturer_dir_name(
    path: &str,
    code_to_name: &mut HashMap<String, (String, String, u8)>,
    mfr_cap: regex::Captures<'_>,
) {
    let code = mfr_cap[1].to_string();
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
            let existing = code_to_name.get(&code).map(|(_, _, p)| *p).unwrap_or(0);
            if existing < 9 {
                code_to_name.insert(
                    code,
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

fn discover_ui_logo_name(
    path_lower: &str,
    code_to_name: &mut HashMap<String, (String, String, u8)>,
    ui_logo_pattern: &Regex,
) {
    let Some(cap) = ui_logo_pattern.captures(path_lower) else {
        return;
    };

    let name = cap[1].to_string();
    let name_title = name
        .chars()
        .enumerate()
        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
        .collect::<String>();

    code_to_name
        .entry(format!("UI_{}", name.to_uppercase()))
        .or_insert((name_title, format!("UI logo: {}", path_lower), 5));
}

fn discover_manufacturer_names_from_paths(
    manifest: &PakManifest,
    code_to_name: &mut HashMap<String, (String, String, u8)>,
    potential_codes: &std::collections::HashSet<&str>,
) {
    let code_in_filename = Regex::new(r"[/_]([A-Z]{3})_[A-Z]{2}[_.]").unwrap();
    let weapon_anim_pattern = Regex::new(r"WeaponAnimation/[^/]+/([A-Za-z]+)/").unwrap();
    let manufacturer_dir_pattern = Regex::new(r"_Manufacturer/([A-Z]{3})/").unwrap();
    let ui_logo_pattern = Regex::new(
        r"ui_art_manu_(?:logomark|logotype|itemcard_logomark|itemcard_logotype)_([a-z]+)",
    )
    .unwrap();

    for item in &manifest.items {
        let path = &item.path;
        discover_weapon_animation_name(
            path,
            code_to_name,
            &weapon_anim_pattern,
            &code_in_filename,
            potential_codes,
        );

        if let Some(mfr_cap) = manufacturer_dir_pattern.captures(path) {
            if potential_codes.contains(&mfr_cap[1]) {
                discover_manufacturer_dir_name(path, code_to_name, mfr_cap);
            }
        }

        let path_lower = path.to_lowercase();
        if path_lower.contains("ui_art_manu") {
            discover_ui_logo_name(&path_lower, code_to_name, &ui_logo_pattern);
        }
    }
}

fn match_ui_to_codes(code_to_name: &mut HashMap<String, (String, String, u8)>) {
    let ui_to_code: Vec<(&str, &str)> = vec![
        ("UI_TORGUE", "TOR"),
        ("UI_VLADOF", "VLA"),
        ("UI_JAKOBS", "JAK"),
        ("UI_MALIWAN", "MAL"),
        ("UI_TEDIORE", "TED"),
        ("UI_DAEDALUS", "DAD"),
        ("UI_ORDER", "ORD"),
        ("UI_RIPPER", "RIP"),
        ("UI_COV", "COV"),
        ("UI_BORG", "BOR"),
    ];

    for (ui_key, code) in ui_to_code {
        if let Some((name, source, priority)) = code_to_name.get(ui_key) {
            let existing = code_to_name.get(code).map(|(_, _, p)| *p).unwrap_or(0);
            if existing < *priority {
                code_to_name.insert(code.to_string(), (name.clone(), source.clone(), *priority));
            }
        }
    }
}

fn build_final_manufacturer_list(
    manifest: &PakManifest,
    code_to_name: &HashMap<String, (String, String, u8)>,
    potential_codes: &std::collections::HashSet<&str>,
) -> HashMap<String, ExtractedManufacturer> {
    let mut manufacturers: HashMap<String, ExtractedManufacturer> = HashMap::new();
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

            if !mfr.paths.contains(&item.path) && mfr.paths.len() < 5 {
                mfr.paths.push(item.path.clone());
            }
        }
    }

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

    manufacturers
}

/// Extract manufacturer names from pak_manifest.json (AUTHORITATIVE)
pub fn extract_manufacturer_names_from_pak(
    pak_manifest_path: &Path,
) -> Result<HashMap<String, ExtractedManufacturer>> {
    let manifest = PakManifest::load(pak_manifest_path)?;

    let potential_codes: std::collections::HashSet<&str> = [
        "BOR", "DAD", "DPL", "JAK", "MAL", "ORD", "RIP", "TED", "TOR", "VLA", "COV", "GRV",
    ]
    .iter()
    .copied()
    .collect();

    let mut code_to_name: HashMap<String, (String, String, u8)> = HashMap::new();
    discover_manufacturer_names_from_paths(&manifest, &mut code_to_name, &potential_codes);
    match_ui_to_codes(&mut code_to_name);

    let manufacturers = build_final_manufacturer_list(&manifest, &code_to_name, &potential_codes);

    Ok(manufacturers)
}
