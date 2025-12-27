//! Class UClass discovery via self-referential pattern
//!
//! In UE5, the UClass for "Class" has ClassPrivate pointing to itself.

use super::super::binary::find_code_bounds;
use super::super::source::MemorySource;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// Discover the "Class" UClass by scanning for self-referential pattern
/// In UE5, the UClass for "Class" has ClassPrivate pointing to itself
/// This doesn't rely on GUObjectArray being correct
pub fn discover_class_uclass(source: &dyn MemorySource) -> Result<usize> {
    let code_bounds = find_code_bounds(source)?;

    eprintln!("Scanning for Class UClass (self-referential pattern)...");

    // Scan writable data sections for the pattern:
    // - Valid vtable at +0x00 (first entry points to code)
    // - ClassPrivate at +0x10 points back to the object itself
    // - NamePrivate at +0x18 contains an FName index for "Class"

    let mut candidates: Vec<usize> = Vec::new();

    for region in source.regions() {
        if !region.is_readable() || !region.is_writable() {
            continue;
        }

        // Focus on data sections in the executable's address space
        if region.start < 0x151000000 || region.start > 0x175000000 {
            continue;
        }

        eprintln!(
            "  Scanning region {:#x}-{:#x} for Class UClass...",
            region.start, region.end
        );

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Scan for self-referential pattern at 8-byte aligned addresses
        for i in (0..data.len().saturating_sub(0x28)).step_by(8) {
            let obj_addr = region.start + i;

            // Read potential vtable pointer
            let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;

            // vtable should be in a valid range (not null, not too low)
            if vtable_ptr < 0x140000000 || vtable_ptr > 0x160000000 {
                continue;
            }

            // ClassPrivate is at +0x10 - check if it's self-referential
            let class_private = LE::read_u64(&data[i + 0x10..i + 0x18]) as usize;

            if class_private != obj_addr {
                continue; // Not self-referential
            }

            // Verify vtable is valid (first entry points to code)
            if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                let first_func = LE::read_u64(&vtable_data) as usize;
                if !code_bounds.contains(first_func) {
                    continue;
                }
            } else {
                continue;
            }

            // Found a candidate!
            eprintln!(
                "  Found self-referential object at {:#x} (vtable={:#x})",
                obj_addr, vtable_ptr
            );
            candidates.push(obj_addr);
        }
    }

    if candidates.is_empty() {
        // Try alternative offsets - maybe ClassPrivate is at a different offset
        eprintln!(
            "  No self-referential UClass found at offset 0x10, trying alternative offsets..."
        );

        for class_offset in [0x08, 0x18, 0x20, 0x28] {
            for region in source.regions() {
                if !region.is_readable() || !region.is_writable() {
                    continue;
                }

                if region.start < 0x151000000 || region.start > 0x175000000 {
                    continue;
                }

                let data = match source.read_bytes(region.start, region.size()) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                for i in (0..data.len().saturating_sub(0x30)).step_by(8) {
                    let obj_addr = region.start + i;
                    let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;

                    if vtable_ptr < 0x140000000 || vtable_ptr > 0x160000000 {
                        continue;
                    }

                    if i + class_offset + 8 > data.len() {
                        continue;
                    }

                    let class_private =
                        LE::read_u64(&data[i + class_offset..i + class_offset + 8]) as usize;

                    if class_private != obj_addr {
                        continue;
                    }

                    if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                        let first_func = LE::read_u64(&vtable_data) as usize;
                        if !code_bounds.contains(first_func) {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    eprintln!(
                        "  Found self-referential at {:#x} with class_offset={:#x}",
                        obj_addr, class_offset
                    );
                    candidates.push(obj_addr);

                    if candidates.len() >= 3 {
                        break;
                    }
                }

                if candidates.len() >= 3 {
                    break;
                }
            }

            if !candidates.is_empty() {
                eprintln!("  Class UClass likely at offset {:#x}", class_offset);
                break;
            }
        }
    }

    if candidates.is_empty() {
        bail!("Could not find Class UClass (self-referential pattern not found)");
    }

    // Return the first candidate
    Ok(candidates[0])
}
