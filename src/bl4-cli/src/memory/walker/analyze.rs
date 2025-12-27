//! Dump Analysis
//!
//! Full memory dump analysis for UE5 discovery.

use crate::memory::binary::find_code_bounds;
use crate::memory::constants::*;
use crate::memory::fname::{FNamePool, FNameReader};
use crate::memory::reflection::discover_uclass_metaclass_exhaustive;
use crate::memory::source::MemorySource;

use anyhow::{bail, Result};

/// Analyze a memory dump: discover UObject layout, FName pool, and UClass metaclass
pub fn analyze_dump(source: &dyn MemorySource) -> Result<()> {
    eprintln!("=== BL4 Dump Analysis ===\n");

    // Step 1: Find code bounds
    eprintln!("Step 1: Finding code bounds from PE header...");
    let code_bounds = find_code_bounds(source)?;
    eprintln!("  Found {} code ranges", code_bounds.ranges.len());

    // Step 2: Discover FNamePool
    eprintln!("\nStep 2: Discovering FNamePool...");

    let pool = match FNamePool::discover(source) {
        Ok(p) => {
            eprintln!("  FNamePool at {:#x}", p.header_addr);
            eprintln!(
                "  {} blocks, cursor at {}",
                p.current_block + 1,
                p.current_cursor
            );
            p
        }
        Err(e) => {
            eprintln!("  ERROR: Could not discover FNamePool: {}", e);
            bail!("FNamePool discovery failed - cannot continue analysis");
        }
    };

    let mut fname_reader = FNameReader::new(pool);

    // Verify FName resolution by finding "Class" and "Object" dynamically
    eprintln!("\nStep 3: Verifying FName resolution...");

    // Find "Class" FName dynamically
    let class_idx = match fname_reader.find_class_index(source) {
        Ok(idx) => {
            eprintln!(
                "  FName 'Class' found at index {} (SDK constant was {})",
                idx, FNAME_CLASS_INDEX
            );
            idx
        }
        Err(e) => {
            eprintln!("  ERROR: Could not find 'Class' FName: {}", e);
            FNAME_CLASS_INDEX // Fall back to SDK constant
        }
    };

    // Find "Object" FName dynamically
    let object_idx = match fname_reader.find_object_index(source) {
        Ok(idx) => {
            eprintln!(
                "  FName 'Object' found at index {} (SDK constant was {})",
                idx, FNAME_OBJECT_INDEX
            );
            idx
        }
        Err(e) => {
            eprintln!("  ERROR: Could not find 'Object' FName: {}", e);
            FNAME_OBJECT_INDEX // Fall back to SDK constant
        }
    };

    // Verify the indices work
    for (idx, expected) in [(class_idx, "Class"), (object_idx, "Object")] {
        match fname_reader.read_name(source, idx) {
            Ok(name) => {
                let status = if name == expected { "OK" } else { "MISMATCH" };
                eprintln!(
                    "  FName {} = \"{}\" (expected \"{}\") [{}]",
                    idx, name, expected, status
                );
            }
            Err(e) => {
                eprintln!("  FName {} = ERROR: {}", idx, e);
            }
        }
    }

    // Step 4: Find UClass metaclass
    eprintln!("\nStep 4: Finding UClass metaclass...");
    match discover_uclass_metaclass_exhaustive(source, &mut fname_reader) {
        Ok(info) => {
            eprintln!("\n=== UClass Metaclass Found ===");
            eprintln!("  Address: {:#x}", info.address);
            eprintln!("  Vtable: {:#x}", info.vtable);
            eprintln!("  ClassPrivate offset: {:#x}", info.class_offset);
            eprintln!("  NamePrivate offset: {:#x}", info.name_offset);
            eprintln!("  FName: {} (\"{}\")", info.fname_index, info.name);

            // Update the constants for future use
            eprintln!("\nRecommended constant updates:");
            eprintln!(
                "  pub const UCLASS_METACLASS_ADDR: usize = {:#x};",
                info.address
            );
            eprintln!(
                "  pub const UCLASS_METACLASS_VTABLE: usize = {:#x};",
                info.vtable
            );
            eprintln!(
                "  pub const UOBJECT_CLASS_OFFSET: usize = {:#x};",
                info.class_offset
            );
            eprintln!(
                "  pub const UOBJECT_NAME_OFFSET: usize = {:#x};",
                info.name_offset
            );
        }
        Err(e) => {
            eprintln!("  Failed: {}", e);
        }
    }

    Ok(())
}
