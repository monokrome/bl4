//! Analysis and dump command handlers
//!
//! Handlers for memory dump analysis, USMAP generation, log monitoring,
//! string scanning, and part extraction.

use crate::memory::{self, MemorySource};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

/// Handle the AnalyzeDump command
///
/// Runs comprehensive analysis on a memory dump.
pub fn handle_analyze_dump(source: &dyn MemorySource) -> Result<()> {
    memory::analyze_dump(source).context("Dump analysis failed")
}

/// Handle the DumpUsmap command
///
/// Generates a USMAP file from memory by extracting UE5 reflection data.
pub fn handle_dump_usmap(source: &dyn MemorySource, output: &Path) -> Result<()> {
    // Step 1: Find GNames pool
    println!("Step 1: Finding GNames pool...");
    let gnames = memory::discover_gnames(source).context("Failed to find GNames pool")?;
    println!("  GNames at: {:#x}", gnames.address);

    // Step 2: Find GUObjectArray
    println!("\nStep 2: Finding GUObjectArray...");
    let guobj_array = memory::discover_guobject_array(source, gnames.address)
        .context("Failed to find GUObjectArray")?;
    println!("  GUObjectArray at: {:#x}", guobj_array.address);
    println!("  Objects ptr: {:#x}", guobj_array.objects_ptr);
    println!("  NumElements: {}", guobj_array.num_elements);

    // Step 3: Walk GUObjectArray to find reflection objects
    println!("\nStep 3: Walking GUObjectArray to find reflection objects...");
    let pool = memory::FNamePool::discover(source).context("Failed to discover FNamePool")?;
    let mut fname_reader = memory::FNameReader::new(pool);
    let reflection_objects = memory::walk_guobject_array(source, &guobj_array, &mut fname_reader)
        .context("Failed to walk GUObjectArray")?;

    // Print summary
    let class_count = reflection_objects
        .iter()
        .filter(|o| o.class_name == "Class")
        .count();
    let struct_count = reflection_objects
        .iter()
        .filter(|o| o.class_name == "ScriptStruct")
        .count();
    let enum_count = reflection_objects
        .iter()
        .filter(|o| o.class_name == "Enum")
        .count();

    println!("\nFound {} reflection objects:", reflection_objects.len());
    println!("  {} UClass", class_count);
    println!("  {} UScriptStruct", struct_count);
    println!("  {} UEnum", enum_count);

    // Print some samples
    println!("\nSample classes:");
    for obj in reflection_objects
        .iter()
        .filter(|o| o.class_name == "Class")
        .take(10)
    {
        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
    }

    println!("\nSample structs:");
    for obj in reflection_objects
        .iter()
        .filter(|o| o.class_name == "ScriptStruct")
        .take(10)
    {
        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
    }

    println!("\nSample enums:");
    for obj in reflection_objects
        .iter()
        .filter(|o| o.class_name == "Enum")
        .take(10)
    {
        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
    }

    // Step 4: Extract properties from each struct/class
    println!("\nStep 4: Extracting properties...");
    let (structs, enums) =
        memory::extract_reflection_data(source, &reflection_objects, &mut fname_reader)
            .context("Failed to extract reflection data")?;

    // Print some sample properties
    println!("\nSample struct properties:");
    for s in structs.iter().filter(|s| !s.properties.is_empty()).take(5) {
        println!(
            "  {} ({}): {} props, super={:?}",
            s.name,
            if s.is_class { "class" } else { "struct" },
            s.properties.len(),
            s.super_name
        );
        for prop in s.properties.iter().take(3) {
            println!(
                "    +{:#x} {} : {} ({:?})",
                prop.offset,
                prop.name,
                prop.type_name,
                prop.struct_type.as_ref().or(prop.enum_type.as_ref())
            );
        }
        if s.properties.len() > 3 {
            println!("    ... and {} more", s.properties.len() - 3);
        }
    }

    println!("\nSample enum values:");
    for e in enums.iter().filter(|e| !e.values.is_empty()).take(5) {
        println!("  {}: {} values", e.name, e.values.len());
        for (name, val) in e.values.iter().take(3) {
            println!("    {} = {}", name, val);
        }
        if e.values.len() > 3 {
            println!("    ... and {} more", e.values.len() - 3);
        }
    }

    // Step 5: Write usmap format
    memory::write_usmap(output, &structs, &enums)?;
    println!("\nWrote usmap file: {}", output.display());

    Ok(())
}

/// Handle the Monitor command
///
/// Monitors a log file in real-time with optional filtering.
pub fn handle_monitor(
    log_file: &Path,
    filter: Option<&str>,
    game_only: bool,
) -> Result<()> {
    println!("Monitoring: {}", log_file.display());
    if let Some(f) = filter {
        println!("Filter: {}", f);
    }
    if game_only {
        println!("Showing only game code addresses (0x140000000+)");
    }
    println!("Press Ctrl+C to stop\n");

    // Tail the log file
    let file = std::fs::File::open(log_file)
        .with_context(|| format!("Failed to open {}", log_file.display()))?;
    let mut reader = std::io::BufReader::new(file);

    // Seek to end first
    reader.seek_relative(
        std::fs::metadata(log_file)
            .map(|m| m.len() as i64)
            .unwrap_or(0),
    )?;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data, wait a bit
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Ok(_) => {
                let line = line.trim();

                // Apply filter
                if let Some(f) = filter {
                    if !line.contains(f) {
                        continue;
                    }
                }

                // Apply game_only filter (addresses 0x140000000+)
                if game_only {
                    if let Some(caller_pos) = line.find("caller=0x") {
                        let addr_str = &line[caller_pos + 9..];
                        if let Some(end) = addr_str.find(|c: char| !c.is_ascii_hexdigit()) {
                            if let Ok(addr) = usize::from_str_radix(&addr_str[..end], 16) {
                                // Skip addresses below game base
                                if addr < 0x140000000 {
                                    continue;
                                }
                            }
                        }
                    }
                }

                println!("{}", line);
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Handle the ScanString command
///
/// Searches for a string in memory and displays context around matches.
pub fn handle_scan_string(
    source: &dyn MemorySource,
    query: &str,
    before: usize,
    after: usize,
    limit: usize,
) -> Result<()> {
    println!("Searching for \"{}\" in memory...", query);
    let search_bytes = query.as_bytes();
    let mask = vec![1u8; search_bytes.len()];

    // Use scan_pattern to find all matches
    let results = memory::scan_pattern(source, search_bytes, &mask)?;

    if results.is_empty() {
        println!("No matches found.");
    } else {
        let show_count = results.len().min(limit);
        println!("Found {} matches, showing {}:", results.len(), show_count);

        for (i, &addr) in results.iter().take(limit).enumerate() {
            println!("\n=== Match {} at {:#x} ===", i + 1, addr);

            // Read context around the match
            let ctx_start = addr.saturating_sub(before);
            let ctx_size = before + search_bytes.len() + after;

            if let Ok(data) = source.read_bytes(ctx_start, ctx_size) {
                // Print hex dump with context
                for j in (0..data.len()).step_by(16) {
                    let line_addr = ctx_start + j;
                    let line_end = (j + 16).min(data.len());
                    let line_bytes = &data[j..line_end];

                    // Hex bytes
                    let hex: String = line_bytes
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<_>>()
                        .join(" ");

                    // ASCII representation
                    let ascii: String = line_bytes
                        .iter()
                        .map(|&b| {
                            if (32..127).contains(&b) {
                                b as char
                            } else {
                                '.'
                            }
                        })
                        .collect();

                    // Mark if this line contains the match
                    let marker = if ctx_start + j <= addr && addr < ctx_start + j + 16 {
                        " <--"
                    } else {
                        ""
                    };
                    println!("{:#010x}: {:<48} {}{}", line_addr, hex, ascii, marker);
                }
            }
        }

        if results.len() > limit {
            println!("\n... and {} more matches", results.len() - limit);
        }
    }

    Ok(())
}

/// Handle the DumpParts command
///
/// Extracts part definitions from memory by scanning for .part_ patterns.
pub fn handle_dump_parts(source: &dyn MemorySource, output: &Path) -> Result<()> {
    println!("Extracting part definitions from memory dump...");

    // Pattern: .part_ - we'll search for this and extract surrounding context
    let pattern = b".part_";
    let mask = vec![1u8; pattern.len()];

    let results = memory::scan_pattern(source, pattern, &mask)?;
    println!(
        "Found {} occurrences of '.part_', analyzing...",
        results.len()
    );

    let mut parts: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for &addr in &results {
        // Read 64 bytes before and 64 after the match
        let ctx_start = addr.saturating_sub(32);
        if let Ok(data) = source.read_bytes(ctx_start, 128) {
            // Find the .part_ position in our buffer
            let rel_offset = addr - ctx_start;

            // Look backwards from .part_ for the prefix (XXX_YY)
            let mut start = rel_offset;
            while start > 0 {
                let c = data[start - 1];
                if c.is_ascii_alphanumeric() || c == b'_' {
                    start -= 1;
                } else {
                    break;
                }
            }

            // Look forward for the rest of the part name
            let mut end = rel_offset + pattern.len();
            while end < data.len() {
                let c = data[end];
                if c.is_ascii_alphanumeric() || c == b'_' {
                    end += 1;
                } else {
                    break;
                }
            }

            // Extract the full part name
            if let Ok(name) = std::str::from_utf8(&data[start..end]) {
                // Validate format: XXX_YY.part_*
                if name.contains('.') && name.len() > 10 {
                    let prefix = name.split('.').next().unwrap_or("");
                    if prefix.len() >= 5 && prefix.contains('_') {
                        parts
                            .entry(prefix.to_string())
                            .or_default()
                            .push(name.to_string());
                    }
                }
            }
        }
    }

    // Deduplicate and sort
    for names in parts.values_mut() {
        names.sort();
        names.dedup();
    }

    // Write JSON using manual formatting (no serde_json dependency needed)
    let mut json = String::from("{\n");
    let mut first_type = true;
    for (prefix, names) in &parts {
        if !first_type {
            json.push_str(",\n");
        }
        first_type = false;
        json.push_str(&format!("  \"{}\": [\n", prefix));
        for (i, name) in names.iter().enumerate() {
            json.push_str(&format!("    \"{}\"", name));
            if i < names.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ]");
    }
    json.push_str("\n}\n");

    std::fs::write(output, &json)?;

    let total_unique: usize = parts.values().map(|v| v.len()).sum();
    println!(
        "Found {} unique part names across {} weapon types",
        total_unique,
        parts.len()
    );
    println!("Written to: {}", output.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    #[test]
    fn test_handle_analyze_dump_empty_source() {
        let source = MockMemorySource::new(vec![], 0x1000);
        // Will fail during analysis since there's no valid data
        let result = handle_analyze_dump(&source);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_dump_usmap_empty_source() {
        let source = MockMemorySource::new(vec![], 0x1000);
        let output = std::path::PathBuf::from("/tmp/test.usmap");
        // Will fail to find GNames
        let result = handle_dump_usmap(&source, &output);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_scan_string_no_matches() {
        let source = MockMemorySource::new(vec![0u8; 1024], 0x1000);
        // Will find no matches but should complete successfully
        let result = handle_scan_string(&source, "not_found", 16, 16, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_scan_string_with_match() {
        let mut data = vec![0u8; 1024];
        // Insert "test" at offset 100
        data[100..104].copy_from_slice(b"test");
        let source = MockMemorySource::new(data, 0x1000);
        let result = handle_scan_string(&source, "test", 16, 16, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_dump_parts_empty_source() {
        let source = MockMemorySource::new(vec![0u8; 1024], 0x1000);
        let output = std::path::PathBuf::from("/tmp/test_parts.json");
        // Should complete with empty results
        let result = handle_dump_parts(&source, &output);
        assert!(result.is_ok());
    }
}
