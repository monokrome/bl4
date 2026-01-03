//! Property type reading
//!
//! Functions for reading property type names from FFieldClass.

#![allow(dead_code)]

use crate::memory::fname::FNameReader;
use crate::memory::source::MemorySource;

use anyhow::Result;
use byteorder::{ByteOrder, LE};

/// Read property type name from FFieldClass
pub fn read_property_type(
    source: &dyn MemorySource,
    field_class_ptr: usize,
    fname_reader: &mut FNameReader,
    debug: bool,
) -> Result<String> {
    if field_class_ptr == 0 {
        return Ok("Unknown".to_string());
    }

    // Read FFieldClass data - in BL4's UE5.4, this is a vtable followed by class data
    // Read 0x180 bytes to find the FName (might be past offset 0x100)
    let class_data = source.read_bytes(field_class_ptr, 0x180)?;

    if debug {
        dump_field_class_debug(&class_data, field_class_ptr);
    }

    // Search for an FName-like value (small index that resolves to *Property)
    // Property type FNames are at low indices: IntProperty ~10, ObjectProperty ~32
    // Scan entire 0x180 byte region
    for offset in (0..0x180).step_by(4) {
        if offset + 4 <= class_data.len() {
            let name_index = LE::read_u32(&class_data[offset..offset + 4]);
            // Property type FNames should be small (< 500) and non-zero
            if name_index > 0 && name_index < 500 {
                if let Ok(name) = fname_reader.read_name(source, name_index) {
                    if name.ends_with("Property") {
                        if debug {
                            eprintln!(
                                "    Found Property type at +{:#x}: idx={}, name='{}'",
                                offset, name_index, name
                            );
                        }
                        return Ok(name);
                    }
                }
            }
        }
    }

    // FFieldClass in BL4's UE5.4 is purely a vtable with no embedded FName.
    // Return a placeholder that will be replaced with actual type during property extraction.
    Ok("_UNKNOWN_TYPE_".to_string())
}

/// Dump FFieldClass data for debugging
fn dump_field_class_debug(class_data: &[u8], field_class_ptr: usize) {
    eprintln!(
        "  FFieldClass at {:#x} (raw dump - 0x180 bytes):",
        field_class_ptr
    );
    for i in 0..24 {
        let offset = i * 16;
        if offset + 16 <= class_data.len() {
            eprint!("    +{:03x}: ", offset);
            for j in 0..16 {
                eprint!("{:02x} ", class_data[offset + j]);
            }
            // Also show as ASCII
            eprint!(" | ");
            for j in 0..16 {
                let b = class_data[offset + j];
                if b >= 0x20 && b < 0x7f {
                    eprint!("{}", b as char);
                } else {
                    eprint!(".");
                }
            }
            eprintln!();
        }
    }
}
