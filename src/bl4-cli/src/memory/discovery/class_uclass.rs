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

    eprintln!("Scanning ALL memory for Class UClass (self-referential pattern)...");

    // Scan ALL readable regions for the pattern:
    // - Valid vtable at +0x00 (first entry points to code)
    // - ClassPrivate at some offset points back to the object itself

    let mut candidates: Vec<(usize, usize)> = Vec::new(); // (addr, class_offset)

    // Try different class offsets
    for class_offset in [0x10usize, 0x08, 0x18, 0x20] {
        for region in source.regions() {
            if !region.is_readable() {
                continue;
            }

            // Read region data (limit to 64MB per region for performance)
            let read_size = region.size().min(64 * 1024 * 1024);
            let data = match source.read_bytes(region.start, read_size) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Scan for self-referential pattern at 8-byte aligned addresses
            for i in (0..data.len().saturating_sub(0x30)).step_by(8) {
                let obj_addr = region.start + i;

                // Read potential vtable pointer
                let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;

                // vtable should be in executable range
                if vtable_ptr < 0x140000000 || vtable_ptr > 0x160000000 {
                    continue;
                }

                if i + class_offset + 8 > data.len() {
                    continue;
                }

                // Check if ClassPrivate is self-referential
                let class_private =
                    LE::read_u64(&data[i + class_offset..i + class_offset + 8]) as usize;

                if class_private != obj_addr {
                    continue;
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
                    "  Found self-referential at {:#x} (vtable={:#x}, class@+{:#x})",
                    obj_addr, vtable_ptr, class_offset
                );

                // Print hex dump for verification
                eprintln!("    Raw bytes:");
                for row in 0..4 {
                    let start = i + row * 16;
                    if start + 16 <= data.len() {
                        let hex: String = data[start..start + 16]
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        eprintln!("    +{:#04x}: {}", row * 16, hex);
                    }
                }

                candidates.push((obj_addr, class_offset));

                if candidates.len() >= 5 {
                    break;
                }
            }

            if candidates.len() >= 5 {
                break;
            }
        }

        if !candidates.is_empty() {
            eprintln!(
                "  Found {} candidates with class_offset={:#x}",
                candidates.len(),
                class_offset
            );
            break;
        }
    }

    if candidates.is_empty() {
        bail!("Could not find Class UClass (self-referential pattern not found)");
    }

    // Return the first candidate
    Ok(candidates[0].0)
}
