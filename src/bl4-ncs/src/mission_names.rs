//! Mission display name extraction from NCS Mission files
//!
//! Extracts the player-facing mission names (e.g., "Recruitment Drive")
//! from NCS `Mission_*.bin` files by reading the `ux_display.text` field
//! of each mission record.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use crate::document::Value;
use crate::{decompress_ncs, is_ncs, parse_ncs_binary_from_reader};

/// A mission name mapping from NCS data
#[derive(Debug, Clone)]
pub struct MissionNameEntry {
    pub internal_name: String,
    pub display_name: String,
}

/// Extract mission display names from a single NCS Mission binary.
pub fn extract_from_binary(data: &[u8]) -> Vec<MissionNameEntry> {
    let decompressed = if is_ncs(data) {
        match decompress_ncs(data) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        }
    } else {
        data.to_vec()
    };

    let doc = match parse_ncs_binary_from_reader(&mut Cursor::new(&decompressed)) {
        Some(d) => d,
        None => return Vec::new(),
    };

    // Only process documents with a "mission" table
    let table = match doc.tables.get("mission") {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    for record in &table.records {
        for entry in &record.entries {
            if !entry.key.starts_with("mission_") {
                continue;
            }

            if let Some(name) = extract_ux_display_text(&entry.value) {
                // Skip entries where display name is just the internal name
                if !name.eq_ignore_ascii_case(&entry.key)
                    && !name.starts_with("Mission_")
                    && !name.starts_with("mission_")
                {
                    entries.push(MissionNameEntry {
                        internal_name: entry.key.clone(),
                        display_name: name,
                    });
                }
            }
        }
    }

    entries
}

/// Walk a Value tree to find ux_display.text and extract the display name.
///
/// The NCS structure is: `{ ux_display: { text: "NexusSerialized, <GUID>, <DisplayName>" } }`
fn extract_ux_display_text(value: &Value) -> Option<String> {
    let map = match value {
        Value::Map(m) => m,
        _ => return None,
    };

    let ux_display = map.get("ux_display")?;
    let ux_map = match ux_display {
        Value::Map(m) => m,
        _ => return None,
    };

    let text = ux_map.get("text")?;
    let text_str = match text {
        Value::Leaf(s) => s,
        _ => return None,
    };

    // Format: "NexusSerialized, <GUID>, <DisplayName>"
    let parts: Vec<&str> = text_str.splitn(3, ", ").collect();
    if parts.len() >= 3 {
        let name = parts[2].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    None
}

/// Extract mission names from all Mission NCS files in a directory.
pub fn extract_from_directory(ncs_dir: &Path) -> Vec<MissionNameEntry> {
    let mut all: HashMap<String, MissionNameEntry> = HashMap::new();

    for entry in walkdir::WalkDir::new(ncs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let fname = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if !fname.starts_with("Mission_") {
            continue;
        }

        if let Ok(data) = std::fs::read(path) {
            for entry in extract_from_binary(&data) {
                all.entry(entry.internal_name.clone()).or_insert(entry);
            }
        }
    }

    let mut result: Vec<MissionNameEntry> = all.into_values().collect();
    result.sort_by(|a, b| a.internal_name.cmp(&b.internal_name));
    result
}

/// Write mission names to a TSV file.
pub fn write_tsv(entries: &[MissionNameEntry], path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "mission\tdisplay_name")?;
    for entry in entries {
        writeln!(f, "{}\t{}", entry.internal_name, entry.display_name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires NCS data files
    fn test_extract_and_write() {
        let ncs_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("share/manifest/ncs");

        if !ncs_dir.exists() {
            return;
        }

        let entries = extract_from_directory(&ncs_dir);
        assert!(!entries.is_empty(), "Should find mission names");

        let out_path = ncs_dir.parent().unwrap().join("missions/mission_names.tsv");
        write_tsv(&entries, &out_path).unwrap();
        eprintln!(
            "Wrote {} mission names to {}",
            entries.len(),
            out_path.display()
        );

        for entry in &entries {
            eprintln!("  {} → {}", entry.internal_name, entry.display_name);
        }
    }
}
