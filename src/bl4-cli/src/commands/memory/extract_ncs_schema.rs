///! Extract NCS field schema from memory
///!
///! Scans FNamePool and attempts to correlate FNames with NCS field hashes.

use crate::memory::{self, MemorySource};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Extract NCS field schema and write to bl4.ncsmap
///
/// This scans the FNamePool in memory and extracts all FName strings,
/// attempting to identify which ones are NCS field names.
pub fn handle_extract_ncs_schema(
    output: &Path,
    dump: Option<&Path>,
) -> Result<()> {
    // Open memory source
    let source: Box<dyn MemorySource> = if let Some(dump_path) = dump {
        Box::new(memory::DumpFile::open(dump_path)
            .context("Failed to open dump file")?)
    } else {
        bail!("ExtractNcsSchema requires a dump file. Use --dump <path> or create a memory dump first.");
    };
    let source = source.as_ref();
    eprintln!("Discovering FNamePool...");
    let pool = memory::FNamePool::discover(source)
        .context("Failed to discover FNamePool")?;

    eprintln!("FNamePool found at {:#x}", pool.header_addr);
    eprintln!("  Blocks: {}", pool.blocks.len());
    eprintln!("  Cursor: {}", pool.current_cursor);

    let mut reader = memory::FNameReader::new(pool.clone());

    // Collect all FNames that might be NCS-related
    let mut field_mappings = HashMap::new();
    let mut scanned = 0;
    let mut found = 0;

    eprintln!("\nScanning FNames for NCS fields...");

    // Scan through a reasonable range of FName indices
    // FName indices are typically 0 to current_cursor
    for fname_index in 0..pool.current_cursor {
        scanned += 1;

        if scanned % 10000 == 0 {
            eprint!("\rScanned {} FNames, found {} potential fields...", scanned, found);
        }

        if let Ok(name) = reader.read_name(source, fname_index) {
            // Filter for likely NCS field names
            // NCS fields are typically lowercase with underscores or PascalCase
            if is_likely_ncs_field(&name) {
                // Compute field hash
                // For now, we'll use FName index as the hash (with alignment)
                // The actual hash algorithm might be different
                let hash = compute_field_hash(fname_index, &name);

                field_mappings.insert(format!("0x{:08x}", hash), name);
                found += 1;
            }
        }
    }

    eprintln!("\n\nFound {} potential NCS field names", found);

    // Write to bl4.ncsmap JSON format
    let ncsmap = json!({
        "version": 1,
        "extracted_from": "Borderlands4.exe memory dump",
        "date": "2026-01-15",
        "total_fnames_scanned": scanned,
        "field_hashes": field_mappings,
    });

    let mut file = File::create(output)
        .with_context(|| format!("Failed to create output file: {}", output.display()))?;

    let json_str = serde_json::to_string_pretty(&ncsmap)
        .context("Failed to serialize ncsmap to JSON")?;

    file.write_all(json_str.as_bytes())
        .with_context(|| format!("Failed to write to {}", output.display()))?;

    eprintln!("Wrote NCS schema to: {}", output.display());
    eprintln!("  {} field mappings extracted", field_mappings.len());

    Ok(())
}

/// Check if an FName is likely an NCS field name
fn is_likely_ncs_field(name: &str) -> bool {
    // Skip empty or very short names
    if name.len() < 2 {
        return false;
    }

    // Skip common UE4/UE5 internal names
    if name.starts_with("Default__")
        || name.starts_with("SKEL_")
        || name.starts_with("REINST_")
        || name.starts_with("BP_")
        || name.contains("/Game/")
        || name.contains("/Engine/")
        || name.contains("/Script/") {
        return false;
    }

    // Include if it looks like a field name:
    // - lowercase_with_underscores
    // - PascalCase
    // - bBooleanField (UE convention)
    // - Contains common NCS terms
    let has_underscore = name.contains('_');
    let is_camel_case = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
    let has_ncs_keywords = name.contains("inv")
        || name.contains("audio")
        || name.contains("weapon")
        || name.contains("part")
        || name.contains("stat")
        || name.contains("attribute")
        || name.contains("aspect");

    has_underscore || is_camel_case || has_ncs_keywords
}

/// Compute field hash from FName index
///
/// Note: The actual NCS field hash algorithm is unknown.
/// This is a placeholder that uses FName index with 0x400 alignment
/// based on observed patterns in the binary.
fn compute_field_hash(fname_index: u32, _name: &str) -> u32 {
    // Observed pattern: hashes are aligned to 0x400
    // This is likely related to FName index encoding
    // For now, we'll preserve the index and align it
    (fname_index << 10) & 0xFFFFFF00
}
