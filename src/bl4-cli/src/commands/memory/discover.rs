//! Memory discovery command handlers
//!
//! Handlers for discovering UE5 memory structures (GNames, GUObjectArray, etc).

use crate::memory::{self, MemorySource};
use anyhow::{Context, Result};
use byteorder::ByteOrder;

/// Handle the Discover command
///
/// Discovers GNames, GUObjectArray, or ClassUClass based on target.
pub fn handle_discover(source: &dyn MemorySource, target: &str) -> Result<()> {
    match target.to_lowercase().as_str() {
        "gnames" | "all" => {
            println!("Searching for GNames pool...");
            match memory::discover_gnames(source) {
                Ok(gnames) => {
                    println!("GNames found at: {:#x}", gnames.address);
                    println!("\nSample names:");
                    for (idx, name) in &gnames.sample_names {
                        println!("  [{}] {}", idx, name);
                    }

                    if target == "all" {
                        println!("\nSearching for GUObjectArray...");
                        match memory::discover_guobject_array(source, gnames.address) {
                            Ok(arr) => {
                                println!("GUObjectArray found at: {:#x}", arr.address);
                                println!("  Objects ptr: {:#x}", arr.objects_ptr);
                                println!("  NumElements: {}", arr.num_elements);
                                println!("  MaxElements: {}", arr.max_elements);
                            }
                            Err(e) => {
                                eprintln!("GUObjectArray not found: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("GNames not found: {}", e);
                }
            }
        }
        "guobjectarray" => {
            // First we need GNames
            println!("Searching for GNames pool first...");
            match memory::discover_gnames(source) {
                Ok(gnames) => {
                    println!("GNames at: {:#x}", gnames.address);
                    println!("\nSearching for GUObjectArray...");
                    match memory::discover_guobject_array(source, gnames.address) {
                        Ok(arr) => {
                            println!("GUObjectArray found at: {:#x}", arr.address);
                            println!("  Objects ptr: {:#x}", arr.objects_ptr);
                            println!("  NumElements: {}", arr.num_elements);
                            println!("  MaxElements: {}", arr.max_elements);
                        }
                        Err(e) => {
                            eprintln!("GUObjectArray not found: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("GNames not found (required for GUObjectArray): {}", e);
                }
            }
        }
        "classuclass" => {
            // Find Class UClass via self-referential pattern
            println!("Searching for Class UClass (self-referential)...");
            match memory::discover_class_uclass(source) {
                Ok(addr) => {
                    println!("Class UClass found at: {:#x}", addr);

                    // Read and dump the UObject structure
                    println!("\nUObject structure dump:");
                    for offset in (0..0x40usize).step_by(8) {
                        if let Ok(val) = source.read_u64(addr + offset) {
                            println!("  +{:#04x}: {:#018x}", offset, val);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Class UClass not found: {}", e);
                }
            }
        }
        _ => {
            eprintln!(
                "Unknown target: {}. Use 'gnames', 'guobjectarray', 'classuclass', or 'all'",
                target
            );
        }
    }
    Ok(())
}

/// Handle the Objects command
///
/// Searches for class names in the FName pool.
pub fn handle_objects(
    source: &dyn MemorySource,
    class: Option<&str>,
    limit: usize,
) -> Result<()> {
    // First discover GNames
    let gnames = memory::discover_gnames(source).context("Failed to find GNames pool")?;

    println!("GNames at: {:#x}", gnames.address);

    // For now, we can only search for class names in the FName pool
    // Full object enumeration requires GUObjectArray
    if let Some(class_name) = class {
        println!("Searching for '{}' in FName pool...", class_name);

        // Search for the class name in memory
        let pattern = class_name.as_bytes();
        let results = memory::scan_pattern(source, pattern, &vec![1u8; pattern.len()])?;

        println!(
            "Found {} occurrences of '{}':",
            results.len().min(limit),
            class_name
        );
        for (i, addr) in results.iter().take(limit).enumerate() {
            println!("  {}: {:#x}", i + 1, addr);

            // Try to read context around the match
            if let Ok(context) = source.read_bytes(addr.saturating_sub(16), 64) {
                // Show as hex + ascii
                print!("      ");
                for byte in &context[..32.min(context.len())] {
                    print!("{:02x} ", byte);
                }
                println!();
                print!("      ");
                for byte in &context[..32.min(context.len())] {
                    let c = *byte as char;
                    if c.is_ascii_graphic() || c == ' ' {
                        print!("{}", c);
                    } else {
                        print!(".");
                    }
                }
                println!();
            }
        }

        if results.len() > limit {
            println!("... and {} more", results.len() - limit);
        }
    } else {
        println!("No class filter specified. Showing FName pool sample:");
        for (idx, name) in &gnames.sample_names {
            println!("  [{}] {}", idx, name);
        }
        println!("\nUse --class <name> to search for specific classes");
        println!("Example: bl4 inject objects --class RarityWeightData");
    }

    Ok(())
}

/// Handle the FindClassUClass command
///
/// Exhaustively searches for Class UClass using self-referential detection.
pub fn handle_find_class_uclass(source: &dyn MemorySource) -> Result<()> {
    // First discover FNamePool to resolve names
    let _gnames = memory::discover_gnames(source).context("Failed to find GNames pool")?;
    let pool = memory::FNamePool::discover(source).context("Failed to discover FNamePool")?;
    let mut fname_reader = memory::FNameReader::new(pool);

    // Get code bounds for vtable validation
    let code_bounds = memory::find_code_bounds(source)?;

    println!("Searching for Class UClass...");
    println!("  Code bounds: {} ranges", code_bounds.ranges.len());

    // Try multiple offset combinations for ClassPrivate and NamePrivate
    // Standard UE5: ClassPrivate=0x10, NamePrivate=0x18
    // BL4 discovered: ClassPrivate=0x18, NamePrivate=0x30
    let offset_combos: &[(usize, usize, &str)] = &[
        (0x18, 0x30, "BL4 (0x18/0x30)"),
        (0x10, 0x18, "Standard UE5"),
        (0x10, 0x30, "Mixed A"),
        (0x20, 0x38, "Offset +8"),
    ];

    for &(class_off, name_off, desc) in offset_combos {
        println!(
            "\nTrying {} - ClassPrivate={:#x}, NamePrivate={:#x}...",
            desc, class_off, name_off
        );

        let mut found_self_refs: Vec<(usize, usize, u32, String)> = Vec::new();
        let mut found_class = false;
        let header_size = name_off + 8;

        for region in source.regions() {
            // Only require readable (data sections may be read-only)
            if !region.is_readable() {
                continue;
            }

            // Include both PE image range AND heap
            // PE: 0x140000000-0x175000000
            // Heap: typically starts around 0x1000000+
            let in_pe = region.start >= 0x140000000 && region.start <= 0x175000000;
            let in_heap = region.start >= 0x1000000 && region.start < 0x140000000;
            if !in_pe && !in_heap {
                continue;
            }

            // Skip very large regions
            if region.size() > 100 * 1024 * 1024 {
                continue;
            }

            let data = match source.read_bytes(region.start, region.size()) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Scan for potential UObjects (8-byte aligned)
            for offset in (0..data.len().saturating_sub(header_size)).step_by(8) {
                let obj_addr = region.start + offset;

                // Check vtable pointer - must be in valid data range
                let vtable_ptr = byteorder::LE::read_u64(&data[offset..offset + 8]) as usize;
                if !(0x140000000..=0x175000000).contains(&vtable_ptr) {
                    continue;
                }

                // Vtable's first entry must point to CODE
                let first_func = match source.read_bytes(vtable_ptr, 8) {
                    Ok(vt) => byteorder::LE::read_u64(&vt) as usize,
                    Err(_) => continue,
                };
                if !code_bounds.contains(first_func) {
                    continue;
                }

                // Check ClassPrivate for self-reference
                let class_ptr = byteorder::LE::read_u64(
                    &data[offset + class_off..offset + class_off + 8],
                ) as usize;
                if class_ptr != obj_addr {
                    continue;
                }

                // Self-referential! Read the name
                let fname_idx =
                    byteorder::LE::read_u32(&data[offset + name_off..offset + name_off + 4]);
                let name = fname_reader
                    .read_name(source, fname_idx)
                    .unwrap_or_else(|_| format!("<idx:{}>", fname_idx));

                found_self_refs.push((obj_addr, vtable_ptr, fname_idx, name.clone()));

                if fname_idx == memory::FNAME_CLASS_INDEX || name == "Class" {
                    println!("\n*** FOUND Class UClass at {:#x} ***", obj_addr);
                    println!("  VTable: {:#x}, vtable[0]: {:#x}", vtable_ptr, first_func);
                    println!("  FName index: {} = \"{}\"", fname_idx, name);
                    found_class = true;
                }
            }
        }

        println!("  Found {} self-referential objects:", found_self_refs.len());
        for (addr, vtable, fname_idx, name) in found_self_refs.iter().take(10) {
            let marker = if *fname_idx == memory::FNAME_CLASS_INDEX {
                " <-- CLASS!"
            } else {
                ""
            };
            println!(
                "    {:#x}: vtable={:#x}, fname={} \"{}\"{}",
                addr, vtable, fname_idx, name, marker
            );
        }

        if found_class {
            println!("\n=== SUCCESS with {} offsets! ===", desc);
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    #[test]
    fn test_handle_discover_unknown_target() {
        let source = MockMemorySource::new(vec![], 0x1000);
        // Should not error, just print message
        let result = handle_discover(&source, "unknown_target");
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_objects_no_class_filter() {
        let source = MockMemorySource::new(vec![], 0x1000);
        // Will fail to find GNames, which is expected
        let result = handle_objects(&source, None, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_find_class_uclass_empty_source() {
        let source = MockMemorySource::new(vec![], 0x1000);
        // Will fail to find GNames, which is expected
        let result = handle_find_class_uclass(&source);
        assert!(result.is_err());
    }
}
