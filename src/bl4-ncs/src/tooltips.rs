//! Tooltip display name extraction from NCS uitooltipdata files
//!
//! Extracts the human-readable display names from tooltip definitions.
//! Each tooltip entry has a `header` field containing the display name
//! in the standard GUID format: "category, GUID, DisplayName".

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use crate::document::Value;
use crate::{decompress_ncs, is_ncs, parse_ncs_binary_from_reader};

/// A tooltip entry mapping an internal key to its display name
#[derive(Debug, Clone)]
pub struct TooltipEntry {
    pub key: String,
    pub display_name: String,
}

/// Extract tooltip entries from a single NCS binary.
pub fn extract_from_binary(data: &[u8]) -> Vec<TooltipEntry> {
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

    let table = match doc.tables.get("uitooltipdata") {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    for record in &table.records {
        for entry in &record.entries {
            let header = match &entry.value {
                Value::Map(m) => match m.get("header") {
                    Some(Value::Leaf(s)) => s,
                    _ => continue,
                },
                _ => continue,
            };

            let display_name = match extract_display_name(header) {
                Some(n) => n,
                None => continue,
            };

            entries.push(TooltipEntry {
                key: entry.key.clone(),
                display_name,
            });
        }
    }

    entries
}

/// Extract display name from a header field.
/// Format: "category, GUID, Display Name"
fn extract_display_name(header: &str) -> Option<String> {
    let parts: Vec<&str> = header.splitn(3, ", ").collect();
    if parts.len() >= 3 {
        let name = parts[2].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Extract tooltip entries from all uitooltipdata NCS files in a directory.
pub fn extract_from_directory(ncs_dir: &Path) -> Vec<TooltipEntry> {
    let mut all: HashMap<String, TooltipEntry> = HashMap::new();

    for entry in walkdir::WalkDir::new(ncs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let fname = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();

        if !fname.starts_with("uitooltipdata") {
            continue;
        }

        if let Ok(data) = std::fs::read(path) {
            for entry in extract_from_binary(&data) {
                // Later patches override earlier ones
                all.insert(entry.key.clone(), entry);
            }
        }
    }

    let mut result: Vec<TooltipEntry> = all.into_values().collect();
    result.sort_by(|a, b| a.key.cmp(&b.key));
    result
}

/// Write tooltip entries to a TSV file.
pub fn write_tsv(entries: &[TooltipEntry], path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "key\tdisplay_name")?;
    for entry in entries {
        writeln!(f, "{}\t{}", entry.key, entry.display_name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_display_name() {
        assert_eq!(
            extract_display_name("tooltips_passives, 158C06FC44CFD933E1E82C951367520F, Grave Sustenance"),
            Some("Grave Sustenance".to_string())
        );
        assert_eq!(
            extract_display_name("tooltips_passives, ABC123, "),
            None
        );
    }

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
        assert!(!entries.is_empty(), "Should find tooltip entries");

        let out_path = ncs_dir.parent().unwrap().join("tooltips.tsv");
        write_tsv(&entries, &out_path).unwrap();

        let passive_count = entries.iter()
            .filter(|e| e.key.contains("passive") || e.key.contains("_P_") || e.key.contains("_p_"))
            .count();
        eprintln!("Wrote {} tooltips ({} passive-related) to {}", entries.len(), passive_count, out_path.display());
    }
}
