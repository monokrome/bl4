//! NCS extract command

use anyhow::{Context, Result};
use bl4_ncs::NcsContent;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::types::{FileInfo, ItemParts, LegendaryComposition, ManufacturerMapping, NexusSerializedEntry, PartIndex};

/// Known weapon manufacturers
const MANUFACTURERS: &[&str] = &["BOR", "DAD", "JAK", "MAL", "ORD", "TED", "TOR", "VLA"];

/// Known weapon types
const WEAPON_TYPES: &[&str] = &["AR", "HW", "PS", "SG", "SM", "SR"];

pub fn extract_by_type(
    path: &Path,
    extract_type: &str,
    output: Option<&Path>,
    json: bool,
) -> Result<()> {
    // Special handling for "parts" extraction (legacy: parts with serial indices)
    if extract_type == "parts" {
        return extract_part_indices(path, output, json);
    }

    // Extract item-to-parts mapping from inv.bin
    if extract_type == "item-parts" {
        return extract_item_parts(path, output, json);
    }

    // Extract NexusSerialized display name mappings
    if extract_type == "names" || extract_type == "nexus-serialized" {
        return extract_nexus_serialized(path, output, json);
    }

    // Extract manufacturer mappings from NexusSerialized
    if extract_type == "manufacturers" {
        return extract_manufacturers(path, output, json);
    }

    // Extract raw string table from NCS file
    if extract_type == "strings" || extract_type == "raw-strings" {
        return extract_raw_strings_cmd(path, output, json);
    }

    // Extract string-numeric pairs from NCS file
    if extract_type == "pairs" || extract_type == "string-numeric" {
        return extract_string_numeric_pairs_cmd(path, output, json);
    }

    // Extract serial indices with item type context
    if extract_type == "serial-indices" || extract_type == "serialindex" {
        return extract_serial_indices_ncs_cmd(path, output, json);
    }

    // Native binary parser extraction
    if extract_type == "binary" || extract_type == "native" {
        return extract_binary_native(path, output, json);
    }

    // V2 binary parser (correct bit-packed algorithm)
    if extract_type == "binary-v2" || extract_type == "v2" {
        return extract_binary_v2(path, output, json);
    }

    // Serial index decoder (scan all inv files)
    if extract_type == "decoder" || extract_type == "serial-decoder" {
        return build_serial_decoder(path, output, json);
    }

    // Manifest export (for parts_database.json)
    if extract_type == "manifest" || extract_type == "parts-manifest" {
        return export_parts_manifest(path, output, json);
    }

    let mut extracted = Vec::new();

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        if !file_path.extension().map(|e| e == "bin").unwrap_or(false) {
            continue;
        }

        if let Ok(data) = fs::read(file_path) {
            if let Some(content) = NcsContent::parse(&data) {
                if content.type_name() == extract_type {
                    extracted.push(FileInfo {
                        path: file_path.to_string_lossy().to_string(),
                        type_name: content.type_name().to_string(),
                        format_code: content.format_code().to_string(),
                        entry_names: content.entry_names().map(|s| s.to_string()).collect(),
                        guids: content.guids().map(|s| s.to_string()).collect(),
                        numeric_values: content
                            .numeric_values()
                            .map(|(s, v)| (s.to_string(), v))
                            .collect(),
                    });
                }
            }
        }
    }

    let output_str = if json {
        serde_json::to_string_pretty(&extracted)?
    } else {
        let mut out = format!("=== Extracted {} entries ===\n\n", extracted.len());
        for info in &extracted {
            out.push_str(&format!("File: {}\n", info.path));
            out.push_str(&format!("Format: {}\n", info.format_code));
            out.push_str("Entries:\n");
            for name in &info.entry_names {
                out.push_str(&format!("  - {}\n", name));
            }
            out.push('\n');
        }
        out
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!(
            "Wrote {} entries to {}",
            extracted.len(),
            output_path.display()
        );
    } else {
        println!("{}", output_str);
    }

    Ok(())
}

/// Extract part serial indices from inv.bin
///
/// The inv.bin NCS file contains part definitions where:
/// - Part names follow pattern: MANU_TYPE_PartName (e.g., BOR_SG_Grip_01)
/// - Serial index immediately follows as a decimal string
fn extract_part_indices(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    // Find inv.bin file
    let inv_path = find_inv_bin(path)?;
    let data = fs::read(&inv_path).context("Failed to read inv.bin")?;

    // Extract null-terminated strings
    let strings = extract_null_strings(&data);

    let mut parts = Vec::new();

    for i in 0..strings.len().saturating_sub(1) {
        let s = &strings[i];

        // Check if this looks like a part name (MANU_TYPE_Name pattern)
        if let Some((manufacturer, weapon_type)) = parse_part_name(s) {
            // Look for numeric index within next 10 strings (indices often have fields between)
            let window_end = (i + 10).min(strings.len());
            for candidate in &strings[(i + 1)..window_end] {

                // Stop if we hit another part name (new record)
                if parse_part_name(candidate).is_some() {
                    break;
                }

                // Check if this is a small integer (serial indices are typically < 1000)
                if let Ok(idx) = candidate.parse::<u32>() {
                    if idx < 1000 {
                        parts.push(PartIndex {
                            part_name: s.clone(),
                            serial_index: idx,
                            manufacturer,
                            weapon_type,
                        });
                        break;
                    }
                }
            }
        }
    }

    // Sort by manufacturer, weapon type, then index
    parts.sort_by(|a, b| {
        (&a.manufacturer, &a.weapon_type, a.serial_index)
            .cmp(&(&b.manufacturer, &b.weapon_type, b.serial_index))
    });

    let output_str = if json {
        serde_json::to_string_pretty(&parts)?
    } else {
        // TSV output
        let mut out = String::from("part_name\tserial_index\tmanufacturer\tweapon_type\n");
        for p in &parts {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                p.part_name, p.serial_index, p.manufacturer, p.weapon_type
            ));
        }
        out
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!(
            "Extracted {} part indices to {}",
            parts.len(),
            output_path.display()
        );
    } else {
        print!("{}", output_str);
    }

    eprintln!("\n# Total: {} parts with serial indices", parts.len());

    Ok(())
}

/// Find inv.bin file in a directory
fn find_inv_bin(path: &Path) -> Result<PathBuf> {
    // If path is a file, use it directly
    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    // Search for inv.bin in directory
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        let name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "inv.bin" {
            return Ok(file_path.to_path_buf());
        }
    }

    anyhow::bail!("inv.bin not found in {}", path.display())
}

/// Extract null-terminated strings from binary data
fn extract_null_strings(data: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();

    for &b in data {
        if b == 0 {
            if !current.is_empty() {
                if let Ok(s) = std::str::from_utf8(&current) {
                    if !s.is_empty() {
                        strings.push(s.to_string());
                    }
                }
                current.clear();
            }
        } else if (32..=126).contains(&b) {
            current.push(b);
        } else {
            current.clear();
        }
    }

    strings
}

/// Parse a part name in MANU_TYPE_Name format
/// Returns (manufacturer, weapon_type) if valid, None otherwise
fn parse_part_name(s: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = s.splitn(3, '_').collect();
    if parts.len() < 3 {
        return None;
    }

    let manufacturer = parts[0];
    let weapon_type = parts[1];

    // Must be a known manufacturer
    if !MANUFACTURERS.contains(&manufacturer) {
        return None;
    }

    // Must be a known weapon type
    if !WEAPON_TYPES.contains(&weapon_type) {
        return None;
    }

    // Rest of the name must be alphanumeric with underscores
    let rest = parts[2];
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }

    Some((manufacturer.to_string(), weapon_type.to_string()))
}

/// Extract complete item-to-parts mapping from inv.bin
///
/// Identifies all item types (weapons, shields, etc.) and their valid parts.
#[allow(clippy::cognitive_complexity)]
fn extract_item_parts(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    let inv_path = find_inv_file(path)?;
    let data = fs::read(&inv_path).context("Failed to read inv file")?;

    let strings = extract_null_strings(&data);

    // Build item type -> parts mapping
    let mut items: BTreeMap<String, ItemParts> = BTreeMap::new();

    // First pass: identify all item types and collect their parts
    for s in &strings {
        // Check for weapon type pattern: MANU_WEAPONTYPE (e.g., DAD_PS)
        if let Some(item_id) = parse_item_type_str(s) {
            items.entry(item_id.clone()).or_insert_with(|| ItemParts {
                item_id,
                parts: Vec::new(),
                legendary_compositions: Vec::new(),
            });
        }

        // Check for non-weapon item types
        if s == "Armor_Shield" {
            items.entry(s.to_string()).or_insert_with(|| ItemParts {
                item_id: s.to_string(),
                parts: Vec::new(),
                legendary_compositions: Vec::new(),
            });
        }

        // Check for part pattern: MANU_WEAPONTYPE_PartName
        if let Some((item_id, _manufacturer, _weapon_type)) = parse_item_part(s) {
            if let Some(item) = items.get_mut(&item_id) {
                if !item.parts.contains(s) {
                    item.parts.push(s.clone());
                }
            }
        }

        // Shield parts: part_ra_* (reactive armor augments)
        if s.starts_with("part_ra_") || s.starts_with("part_core_") {
            if let Some(item) = items.get_mut("Armor_Shield") {
                if !item.parts.contains(s) {
                    item.parts.push(s.clone());
                }
            }
        }
    }

    // Second pass: identify legendary compositions
    let mut current_comp: Option<String> = None;
    let mut current_uni: Option<String> = None;

    for s in &strings {
        if s.starts_with("comp_05_legendary_") {
            // Reset for new composition
            current_comp.take();
            current_comp = Some(s.clone());
            current_uni = None;
        } else if s.starts_with("uni_") && current_comp.is_some() {
            current_uni = Some(s.clone());
        } else if s.starts_with("part_") && current_comp.is_some() {
            // This is a mandatory part for the current legendary composition
            let comp_name = current_comp.clone().unwrap();

            // Find which item this composition belongs to by matching the part
            for item in items.values_mut() {
                // Check if this part or a variant exists in the item's parts
                if item.parts.iter().any(|p| {
                    p.contains(&s.replace("part_", ""))
                        || s.contains(&item.item_id.replace("_", ""))
                }) {
                    // Add or update the composition
                    if let Some(existing) = item
                        .legendary_compositions
                        .iter_mut()
                        .find(|c| c.name == comp_name)
                    {
                        if !existing.mandatory_parts.contains(s) {
                            existing.mandatory_parts.push(s.clone());
                        }
                    } else {
                        item.legendary_compositions.push(LegendaryComposition {
                            name: comp_name.clone(),
                            unique_name: current_uni.clone(),
                            mandatory_parts: vec![s.clone()],
                        });
                    }
                    break;
                }
            }
        }
    }

    // Sort parts within each item
    for item in items.values_mut() {
        item.parts.sort();
    }

    let items_vec: Vec<_> = items.into_values().collect();

    let output_str = if json {
        serde_json::to_string_pretty(&items_vec)?
    } else {
        let mut out = String::new();
        for item in &items_vec {
            out.push_str(&format!(
                "=== {} ({} parts) ===\n",
                item.item_id,
                item.parts.len()
            ));
            for part in &item.parts {
                out.push_str(&format!("  {}\n", part));
            }
            if !item.legendary_compositions.is_empty() {
                out.push_str("  Legendary Compositions:\n");
                for comp in &item.legendary_compositions {
                    out.push_str(&format!("    {} ", comp.name));
                    if let Some(ref uni) = comp.unique_name {
                        out.push_str(&format!("({})", uni));
                    }
                    out.push('\n');
                    for part in &comp.mandatory_parts {
                        out.push_str(&format!("      -> {}\n", part));
                    }
                }
            }
            out.push('\n');
        }
        out
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        let total_parts: usize = items_vec.iter().map(|i| i.parts.len()).sum();
        println!(
            "Extracted {} items with {} total parts to {}",
            items_vec.len(),
            total_parts,
            output_path.display()
        );
    } else {
        print!("{}", output_str);
    }

    let total_parts: usize = items_vec.iter().map(|i| i.parts.len()).sum();
    eprintln!(
        "\n# Total: {} items, {} parts",
        items_vec.len(),
        total_parts
    );

    Ok(())
}

/// Find an inv*.bin file in a directory
fn find_inv_file(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        let name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Match inv.bin or Nexus-Data-inv*.bin
        if name == "inv.bin" || (name.contains("-inv") && name.ends_with(".bin")) {
            return Ok(file_path.to_path_buf());
        }
    }

    anyhow::bail!("inv.bin not found in {}", path.display())
}

/// Parse an item type identifier (e.g., "DAD_PS", "BOR_SG")
/// Returns the full identifier string
fn parse_item_type_str(s: &str) -> Option<String> {
    parse_item_type(s).map(|(mfr, wep)| format!("{}_{}", mfr, wep))
}

/// Parse an item type identifier (e.g., "DAD_PS", "BOR_SG")
/// Returns (manufacturer_code, weapon_type_code) tuple
fn parse_item_type(s: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = s.split('_').collect();
    if parts.len() != 2 {
        return None;
    }

    let manufacturer = parts[0];
    let weapon_type = parts[1];

    if !MANUFACTURERS.contains(&manufacturer) {
        return None;
    }

    if !WEAPON_TYPES.contains(&weapon_type) {
        return None;
    }

    Some((manufacturer.to_string(), weapon_type.to_string()))
}

/// Parse an item part (e.g., "DAD_PS_Barrel_01")
/// Returns (item_id, manufacturer, weapon_type) if valid
fn parse_item_part(s: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = s.splitn(3, '_').collect();
    if parts.len() < 3 {
        return None;
    }

    let manufacturer = parts[0];
    let weapon_type = parts[1];

    if !MANUFACTURERS.contains(&manufacturer) {
        return None;
    }

    if !WEAPON_TYPES.contains(&weapon_type) {
        return None;
    }

    let rest = parts[2];
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }

    let item_id = format!("{}_{}", manufacturer, weapon_type);
    Some((item_id, manufacturer.to_string(), weapon_type.to_string()))
}

/// Extract NexusSerialized display name mappings from inv.bin
///
/// Pattern in NCS data:
///   {MFR_TYPE}\0 NexusSerialized, {GUID}, {Display Name}\0
/// Example:
///   BOR_SG\0 NexusSerialized, 4D3ECE4B..., Ripper Shotgun\0
fn extract_nexus_serialized(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    let inv_path = find_inv_file(path)?;
    let data = fs::read(&inv_path).context("Failed to read inv file")?;

    let strings = extract_null_strings(&data);

    // First pass: build manufacturer code -> display name mapping
    // by looking at strings before NexusSerialized entries
    let mfr_mapping = extract_manufacturer_mapping(&strings);

    let mut entries = Vec::new();

    for (i, s) in strings.iter().enumerate() {
        if let Some(mut entry) = parse_nexus_serialized(s) {
            // Look at preceding strings for item type code (e.g., "BOR_SG")
            let search_range = i.saturating_sub(5)..i;
            for j in search_range.rev() {
                // Try strict weapon type match first (BOR_SG -> BOR, SG)
                if let Some((mfr_code, wep_type)) = parse_item_type(&strings[j]) {
                    entry.manufacturer_code = Some(mfr_code);
                    entry.weapon_type = Some(weapon_type_display_name(&wep_type));
                    break;
                }
                // Fall back to just manufacturer code (BOR_Enhancement -> BOR)
                if let Some(mfr_code) = parse_manufacturer_code(&strings[j]) {
                    entry.manufacturer_code = Some(mfr_code);
                    break;
                }
            }

            // If we didn't get manufacturer from context, try parsing from display name
            if entry.manufacturer_code.is_none() {
                let (mfr_code, wep_type) = parse_display_name_with_mapping(&entry.display_name, &mfr_mapping);
                entry.manufacturer_code = mfr_code;
                entry.weapon_type = wep_type;
            }

            // Avoid duplicates
            if !entries.iter().any(|e: &NexusSerializedEntry| e.guid == entry.guid) {
                entries.push(entry);
            }
        }
    }

    // Sort by display name
    entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    let output_str = if json {
        serde_json::to_string_pretty(&entries)?
    } else {
        let mut out = String::from("guid\tdisplay_name\tmanufacturer_code\tweapon_type\n");
        for e in &entries {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                e.guid,
                e.display_name,
                e.manufacturer_code.as_deref().unwrap_or(""),
                e.weapon_type.as_deref().unwrap_or("")
            ));
        }
        out
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!(
            "Extracted {} NexusSerialized entries to {}",
            entries.len(),
            output_path.display()
        );
    } else {
        print!("{}", output_str);
    }

    eprintln!("\n# Total: {} NexusSerialized entries", entries.len());

    Ok(())
}

/// Weapon type keywords used to identify weapon NexusSerialized entries
const WEAPON_TYPE_KEYWORDS: &[(&str, &str)] = &[
    ("Assault Rifle", "AR"),
    ("Heavy Weapon", "HW"),
    ("Pistol", "PS"),
    ("Shotgun", "SG"),
    ("SMG", "SM"),
    ("Sniper", "SR"),
];

/// Extract manufacturer code -> display name mapping from NCS strings
/// by finding patterns like: BOR_SG\0 NexusSerialized, ..., Ripper Shotgun\0
///
/// Only considers NexusSerialized entries that clearly follow the pattern
/// "{Manufacturer Name} {Weapon Type}" (e.g., "Ripper Shotgun")
/// and verifies the weapon type matches the context.
fn extract_manufacturer_mapping(strings: &[String]) -> BTreeMap<String, String> {
    let mut mapping: BTreeMap<String, String> = BTreeMap::new();

    for (i, s) in strings.iter().enumerate() {
        if let Some(entry) = parse_nexus_serialized(s) {
            // Check if this is a weapon type NexusSerialized entry
            // by looking for weapon type keywords at the end of the display name
            let wep_type_match = WEAPON_TYPE_KEYWORDS
                .iter()
                .find(|(keyword, _)| entry.display_name.ends_with(keyword));

            if let Some((keyword, wep_code)) = wep_type_match {
                // Extract manufacturer name by removing weapon type suffix
                let mfr_name = entry
                    .display_name
                    .strip_suffix(keyword)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());

                if let Some(mfr_name) = mfr_name {
                    // Look at preceding strings for item type code that matches
                    let search_range = i.saturating_sub(5)..i;
                    for j in search_range.rev() {
                        // Try to parse as MFR_WEAPONTYPE
                        if let Some((mfr_code, ctx_wep_code)) = parse_item_type(&strings[j]) {
                            // Verify the weapon type matches
                            if ctx_wep_code == *wep_code {
                                mapping.entry(mfr_code).or_insert_with(|| mfr_name.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    mapping
}

/// Parse a manufacturer code from a string like "BOR_SG", "DAD_PS", "BOR_Enhancement"
/// Returns just the manufacturer code (e.g., "BOR", "DAD")
fn parse_manufacturer_code(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split('_').collect();
    if parts.len() < 2 {
        return None;
    }

    let manufacturer = parts[0];

    // Check if it's a known manufacturer code
    if MANUFACTURERS.contains(&manufacturer) {
        return Some(manufacturer.to_string());
    }

    None
}

/// Convert weapon type code to display name
/// e.g., "SG" -> "Shotgun", "PS" -> "Pistol"
fn weapon_type_display_name(code: &str) -> String {
    match code {
        "AR" => "Assault Rifle".to_string(),
        "HW" => "Heavy Weapon".to_string(),
        "PS" => "Pistol".to_string(),
        "SG" => "Shotgun".to_string(),
        "SM" => "SMG".to_string(),
        "SR" => "Sniper".to_string(),
        _ => code.to_string(),
    }
}

/// Parse a NexusSerialized string
/// Format: "NexusSerialized, {GUID}, {Display Name}"
fn parse_nexus_serialized(s: &str) -> Option<NexusSerializedEntry> {
    if !s.starts_with("NexusSerialized, ") {
        return None;
    }

    let rest = &s[17..]; // Skip "NexusSerialized, "
    let parts: Vec<&str> = rest.splitn(2, ", ").collect();
    if parts.len() != 2 {
        return None;
    }

    let guid = parts[0].to_string();
    let display_name = parts[1].to_string();

    Some(NexusSerializedEntry {
        guid,
        display_name,
        manufacturer_code: None,
        weapon_type: None,
    })
}

/// Parse display name using extracted manufacturer mapping
fn parse_display_name_with_mapping(name: &str, mfr_mapping: &BTreeMap<String, String>) -> (Option<String>, Option<String>) {
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.is_empty() {
        return (None, None);
    }

    // Check if first word matches any known manufacturer display name
    let manufacturer_code = mfr_mapping
        .iter()
        .find(|(_, display)| display.as_str() == words[0])
        .map(|(code, _)| code.clone());

    // If we found a manufacturer, the rest is the weapon/item type
    let weapon_type = if manufacturer_code.is_some() && words.len() > 1 {
        Some(words[1..].join(" "))
    } else {
        None
    };

    (manufacturer_code, weapon_type)
}

/// Extract manufacturer mappings from NexusSerialized entries
fn extract_manufacturers(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    let inv_path = find_inv_file(path)?;
    let data = fs::read(&inv_path).context("Failed to read inv file")?;

    let strings = extract_null_strings(&data);

    // Use the manufacturer mapping extraction
    let manufacturers = extract_manufacturer_mapping(&strings);

    let mappings: Vec<ManufacturerMapping> = manufacturers
        .into_iter()
        .map(|(code, name)| ManufacturerMapping { code, name })
        .collect();

    let output_str = if json {
        // Output as object for direct use in manifest
        let obj: serde_json::Map<String, serde_json::Value> = mappings
            .iter()
            .map(|m| {
                (
                    m.code.clone(),
                    serde_json::json!({"code": m.code, "name": m.name}),
                )
            })
            .collect();
        serde_json::to_string_pretty(&obj)?
    } else {
        let mut out = String::from("code\tname\n");
        for m in &mappings {
            out.push_str(&format!("{}\t{}\n", m.code, m.name));
        }
        out
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!(
            "Extracted {} manufacturer mappings to {}",
            mappings.len(),
            output_path.display()
        );
    } else {
        print!("{}", output_str);
    }

    eprintln!("\n# Total: {} manufacturers", mappings.len());

    Ok(())
}

/// Extract raw string table from NCS file
fn extract_raw_strings_cmd(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    use bl4_ncs::inventory::{extract_raw_strings, raw_strings_to_tsv};

    let file_path = find_inv_file(path)?;
    let data = fs::read(&file_path).context("Failed to read file")?;

    let strings = extract_raw_strings(&data);

    let output_str = if json {
        serde_json::to_string_pretty(&strings)?
    } else {
        raw_strings_to_tsv(&strings)
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!(
            "Extracted {} strings to {}",
            strings.len(),
            output_path.display()
        );
    } else {
        print!("{}", output_str);
    }

    eprintln!("\n# Total: {} strings", strings.len());

    Ok(())
}

/// Extract string-numeric pairs from NCS file
fn extract_string_numeric_pairs_cmd(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    use bl4_ncs::inventory::{extract_string_numeric_pairs, string_numeric_pairs_to_tsv};

    let file_path = find_inv_file(path)?;
    let data = fs::read(&file_path).context("Failed to read file")?;

    let pairs = extract_string_numeric_pairs(&data);

    let output_str = if json {
        serde_json::to_string_pretty(&pairs)?
    } else {
        string_numeric_pairs_to_tsv(&pairs)
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!(
            "Extracted {} string-numeric pairs to {}",
            pairs.len(),
            output_path.display()
        );
    } else {
        print!("{}", output_str);
    }

    eprintln!("\n# Total: {} pairs", pairs.len());

    Ok(())
}

/// Extract serial indices using the new NCS binary parser
fn extract_serial_indices_ncs_cmd(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    use bl4_ncs::{decompress_ncs, is_ncs};

    let file_path = find_inv_file(path)?;
    let raw_data = fs::read(&file_path).context("Failed to read file")?;

    let data = if is_ncs(&raw_data) {
        decompress_ncs(&raw_data).context("Failed to decompress NCS data")?
    } else {
        raw_data
    };

    let doc = bl4_ncs::parse::parse(&data)
        .context("Failed to parse NCS binary data")?;

    eprintln!("Tables: {}", doc.tables.len());
    for (name, table) in &doc.tables {
        eprintln!("  {}: {} records, {} deps", name, table.records.len(), table.deps.len());
    }

    let serial_indices = bl4_ncs::document::extract_serial_indices(&doc);

    eprintln!("Extracted {} serial indices", serial_indices.len());

    let output_str = if json {
        serde_json::to_string_pretty(&serial_indices)?
    } else {
        let mut lines = vec!["table\tdep_table\tpart\tserial_index".to_string()];
        for si in &serial_indices {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                si.table_name, si.dep_table, si.part_name, si.index
            ));
        }
        lines.join("\n")
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!("Extracted {} serial indices to {}", serial_indices.len(), output_path.display());
    } else {
        println!("{}", output_str);
    }

    Ok(())
}

/// Extract using the new binary parser (structured output)
fn extract_binary_native(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    use bl4_ncs::{decompress_ncs, is_ncs};

    let inv_path = find_inv_file(path)?;
    let raw_data = fs::read(&inv_path).context("Failed to read inv file")?;

    let data = if is_ncs(&raw_data) {
        decompress_ncs(&raw_data).context("Failed to decompress NCS data")?
    } else {
        raw_data
    };

    let doc = bl4_ncs::parse::parse(&data)
        .context("Failed to parse NCS binary data")?;

    eprintln!("Tables: {}", doc.tables.len());
    for (name, table) in &doc.tables {
        eprintln!("  {}: {} records, {} deps", name, table.records.len(), table.deps.len());
    }

    if json {
        let output_str = serde_json::to_string_pretty(&doc)?;
        if let Some(output_path) = output {
            fs::write(output_path, &output_str)?;
            println!("Wrote parsed document to {}", output_path.display());
        } else {
            println!("{}", output_str);
        }
    } else {
        let serial_indices = bl4_ncs::document::extract_serial_indices(&doc);
        let mut lines = vec!["table\tdep_table\tpart\tserial_index".to_string()];
        for si in &serial_indices {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                si.table_name, si.dep_table, si.part_name, si.index
            ));
        }
        let output_str = lines.join("\n");
        if let Some(output_path) = output {
            fs::write(output_path, &output_str)?;
            println!("Wrote {} serial indices to {}", serial_indices.len(), output_path.display());
        } else {
            println!("{}", output_str);
        }
    }

    Ok(())
}

/// Extract using the new binary parser (alias for "binary" command)
fn extract_binary_v2(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    extract_binary_native(path, output, json)
}

/// Build complete serial index decoder by scanning all inv*.bin files
fn build_serial_decoder(path: &Path, output: Option<&Path>, json: bool) -> Result<()> {
    use bl4_ncs::{decompress_ncs, is_ncs};

    let mut all_indices = Vec::new();
    let mut files_processed = 0;

    eprintln!("Scanning for inv*.bin files...");
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !filename.contains("-inv") || !filename.ends_with(".bin") || filename.contains("inventory_container") {
            continue;
        }

        eprintln!("Processing: {}", file_path.display());

        let raw_data = match fs::read(file_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  Warning: Failed to read file: {}", e);
                continue;
            }
        };

        let data = if is_ncs(&raw_data) {
            match decompress_ncs(&raw_data) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("  Warning: Failed to decompress: {}", e);
                    continue;
                }
            }
        } else {
            raw_data
        };

        let doc = match bl4_ncs::parse::parse(&data) {
            Some(d) => d,
            None => {
                eprintln!("  Warning: Failed to parse");
                continue;
            }
        };

        let indices = bl4_ncs::document::extract_serial_indices(&doc);
        eprintln!("  Found {} serial indices", indices.len());
        all_indices.extend(indices);
        files_processed += 1;
    }

    // Deduplicate by (part_name, index)
    let mut seen = std::collections::HashSet::new();
    all_indices.retain(|e| seen.insert((e.part_name.clone(), e.index)));

    eprintln!("\nProcessed {} files, found {} unique serial indices",
        files_processed, all_indices.len());

    let output_str = if json {
        serde_json::to_string_pretty(&all_indices)?
    } else {
        let mut lines = vec!["table\tdep_table\tpart\tserial_index".to_string()];
        for si in &all_indices {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                si.table_name, si.dep_table, si.part_name, si.index
            ));
        }
        lines.join("\n")
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!("Wrote {} serial indices to {}", all_indices.len(), output_path.display());
    } else {
        println!("{}", output_str);
    }

    Ok(())
}

/// Known manufacturer code → display name mapping
/// Includes both short codes (bor, dad) from weapon parts and
/// long codes (borg, daedalus) from non-weapon NCS entries
const MANUFACTURER_NAMES: &[(&str, &str)] = &[
    ("bor", "Ripper"),
    ("borg", "Ripper"),
    ("dad", "Daedalus"),
    ("daedalus", "Daedalus"),
    ("jak", "Jakobs"),
    ("jakobs", "Jakobs"),
    ("mal", "Maliwan"),
    ("maliwan", "Maliwan"),
    ("ord", "Order"),
    ("order", "Order"),
    ("ted", "Tediore"),
    ("tediore", "Tediore"),
    ("tor", "Torgue"),
    ("torgue", "Torgue"),
    ("vla", "Vladof"),
    ("vladof", "Vladof"),
    ("atl", "Atlas"),
    ("cov", "CoV"),
    ("hyp", "Hyperion"),
];

/// Known weapon type code → display name mapping
const WEAPON_TYPE_NAMES: &[(&str, &str)] = &[
    ("ar", "AR"),
    ("hw", "Heavy Weapon"),
    ("ps", "Pistol"),
    ("sg", "Shotgun"),
    ("sm", "SMG"),
    ("sr", "Sniper"),
];

/// Convert an NCS entry key (e.g., "dad_ps", "armor_shield") to a human-readable name
fn humanize_category_key(key: &str) -> String {
    let lower = key.to_lowercase();
    let parts: Vec<&str> = lower.split('_').collect();

    // Try manufacturer + weapon type pattern (e.g., "dad_ps" → "Daedalus Pistol")
    if parts.len() == 2 {
        let mfr = MANUFACTURER_NAMES.iter().find(|(code, _)| *code == parts[0]);
        let wep = WEAPON_TYPE_NAMES.iter().find(|(code, _)| *code == parts[1]);

        if let (Some((_, mfr_name)), Some((_, wep_name))) = (mfr, wep) {
            return format!("{} {}", mfr_name, wep_name);
        }
    }

    // Replace manufacturer codes anywhere in the name, then title-case
    let words: Vec<String> = parts.iter().map(|w| {
        // Check if this word is a manufacturer code
        if let Some((_, display)) = MANUFACTURER_NAMES.iter().find(|(code, _)| code == w) {
            return display.to_string();
        }
        // Check if this word is a weapon type code
        if let Some((_, display)) = WEAPON_TYPE_NAMES.iter().find(|(code, _)| code == w) {
            return display.to_string();
        }
        // Title-case the word
        let mut chars = w.chars();
        match chars.next() {
            Some(c) => {
                let upper: String = c.to_uppercase().collect();
                format!("{}{}", upper, chars.as_str())
            }
            None => String::new(),
        }
    }).collect();

    // Fix common compound words
    let result = words.join(" ");
    result
        .replace("Classmod", "Class Mod")
        .replace("Repairkit", "Repair Kit")
        .replace("Heavyweapon", "Heavy Weapon")
}

/// Human-readable name for a dep_table identifier
fn humanize_dep_table(dep_table: &str) -> String {
    dep_table
        .split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .replace("Inv Comp", "Rarity Component")
        .replace("Ele", "Element")
}

/// Parse an NCS file, handling decompression if needed
fn parse_ncs_file(path: &Path) -> Option<bl4_ncs::document::Document> {
    use bl4_ncs::{decompress_ncs, is_ncs};

    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let raw_data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  Skipping {}: {}", filename, e);
            return None;
        }
    };

    let data = if is_ncs(&raw_data) {
        match decompress_ncs(&raw_data) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  Skipping {}: decompress failed: {}", filename, e);
                return None;
            }
        }
    } else {
        raw_data
    };

    let doc = bl4_ncs::parse::parse(&data);
    if doc.is_none() {
        eprintln!("  Skipping {}: parse failed", filename);
    }
    doc
}

/// Export parts manifest for integration into parts_database.json
///
/// Scans all inv*.bin files and extracts categorized parts and category names.
/// Each NCS entry with a serialindex defines a category, and its
/// dep_entries provide the parts with their serial indices.
/// Also extracts shared cross-category parts from dep tables (elements,
/// stat mods, rarity components, etc.).
///
/// Produces two files:
/// - parts_database.json (parts with category/index/name)
/// - category_names.json (category ID → human-readable name)
fn export_parts_manifest(path: &Path, output: Option<&Path>, _json: bool) -> Result<()> {
    #[derive(Debug, serde::Serialize)]
    struct PartsManifest {
        version: u32,
        source: String,
        parts: Vec<ManifestPartEntry>,
    }

    #[derive(Debug, serde::Serialize)]
    struct ManifestPartEntry {
        category: u32,
        index: u32,
        name: String,
    }

    let mut all_parts = Vec::new();
    let mut all_category_names: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    let mut shared_dep_tables: BTreeMap<String, u32> = BTreeMap::new();
    let mut files_processed = 0;

    eprintln!("Scanning for inv*.bin files...");
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !filename.contains("-inv") || !filename.ends_with(".bin") || filename.contains("inventory_container") {
            continue;
        }

        let Some(doc) = parse_ncs_file(file_path) else {
            continue;
        };

        let parts = bl4_ncs::document::extract_categorized_parts(&doc);
        let shared = bl4_ncs::document::extract_shared_parts(&doc);
        let cat_names = bl4_ncs::document::extract_category_names(&doc);
        eprintln!(
            "Processing {} ({} parts, {} shared, {} categories)...",
            filename, parts.len(), shared.len(), cat_names.len()
        );

        for p in parts {
            all_parts.push(ManifestPartEntry {
                category: p.category,
                index: p.index,
                name: p.name,
            });
        }

        for sp in shared {
            let next_id = 10001 + shared_dep_tables.len() as u32;
            let category = *shared_dep_tables
                .entry(sp.dep_table.clone())
                .or_insert(next_id);
            all_parts.push(ManifestPartEntry {
                category,
                index: sp.index,
                name: sp.name,
            });
        }

        // Collect category names: entries with parts get priority names,
        // but also collect ALL entry names from non-custom files for quest
        // items and other non-modular items (inv_custom reuses the same
        // index space for cosmetics so we skip those)
        let is_custom = filename.contains("inv_custom");
        if is_custom {
            for (cat_id, ncs_key) in cat_names {
                all_category_names.entry(cat_id).or_insert(ncs_key);
            }
        } else {
            let all_names = bl4_ncs::document::extract_all_entry_names(&doc);
            for (cat_id, ncs_key) in all_names {
                all_category_names.entry(cat_id).or_insert(ncs_key);
            }
        }

        files_processed += 1;
    }

    // Deduplicate by (category, index, name)
    let mut seen = std::collections::HashSet::new();
    all_parts.retain(|e| seen.insert((e.category, e.index, e.name.clone())));

    all_parts.sort_by_key(|p| (p.category, p.index));

    let categories_with_parts: std::collections::HashSet<u32> =
        all_parts.iter().map(|p| p.category).collect();

    eprintln!("\nExtraction complete:");
    eprintln!("  Files processed: {}", files_processed);
    eprintln!("  Unique parts: {}", all_parts.len());
    eprintln!("  Categories named: {}", all_category_names.len());

    let manifest = PartsManifest {
        version: 2,
        source: "NCS inv*.bin extraction".to_string(),
        parts: all_parts,
    };

    let output_str = serde_json::to_string_pretty(&manifest)?;

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!("Wrote manifest with {} parts to {}", manifest.parts.len(), output_path.display());

        // Write category_names.json alongside parts_database.json
        let cat_names_path = output_path.with_file_name("category_names.json");
        let mut humanized: BTreeMap<String, String> = BTreeMap::new();
        for (cat_id, ncs_key) in &all_category_names {
            humanized.insert(cat_id.to_string(), humanize_category_key(ncs_key));
        }

        for (dep_table, cat_id) in &shared_dep_tables {
            if categories_with_parts.contains(cat_id) {
                humanized.entry(cat_id.to_string())
                    .or_insert_with(|| humanize_dep_table(dep_table));
            }
        }

        let cat_obj = serde_json::json!({ "categories": humanized });
        let cat_str = serde_json::to_string_pretty(&cat_obj)?;
        fs::write(&cat_names_path, &cat_str)?;
        println!("Wrote {} category names to {}", humanized.len(), cat_names_path.display());
    } else {
        println!("{}", output_str);
    }

    Ok(())
}
