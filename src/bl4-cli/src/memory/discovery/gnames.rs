//! GNames pool discovery
//!
//! Discovers the GNames pool by searching for the characteristic
//! "None" + "ByteProperty" pattern at the start of the name pool.

use super::super::binary::scan_pattern;
use super::super::source::MemorySource;
use super::super::ue5::GNamesPool;

use anyhow::{bail, Result};

/// Discover GNames pool by searching for the characteristic "None" + "ByteProperty" pattern
pub fn discover_gnames(source: &dyn MemorySource) -> Result<GNamesPool> {
    // GNames starts with FNameEntry for "None" followed by "ByteProperty"
    // FNameEntry format in UE5: length_byte (low 6 bits + flags), string bytes
    // "None" with typical flags: 1e 01 4e 6f 6e 65 (length=4, flags, "None")
    // Then "ByteProperty": 10 03 42 79 74 65 50 72 6f 70 65 72 74 79

    // Search for "None" followed by "ByteProperty"
    let pattern = b"\x1e\x01None\x10\x03ByteProperty";
    let mask = vec![1u8; pattern.len()];

    let results = scan_pattern(source, pattern, &mask)?;

    if results.is_empty() {
        // Try alternative pattern without exact length bytes
        let alt_pattern: &[u8] = b"None";
        let alt_mask = vec![1u8; alt_pattern.len()];
        let alt_results = scan_pattern(source, alt_pattern, &alt_mask)?;

        // Filter to find ones followed by ByteProperty
        for addr in alt_results {
            if addr < 2 {
                continue;
            }
            // Check if "ByteProperty" follows within ~20 bytes
            if let Ok(data) = source.read_bytes(addr.saturating_sub(2), 64) {
                if let Some(_pos) = data.windows(12).position(|w| w == b"ByteProperty") {
                    // Found it! The pool starts before "None"
                    let gnames_addr = addr - 2; // Account for length/flags bytes

                    // Read some sample names
                    let mut sample_names = Vec::new();
                    sample_names.push((0, "None".to_string()));
                    sample_names.push((1, "ByteProperty".to_string()));

                    // Try to read more names from the pool
                    if let Ok(pool_data) = source.read_bytes(gnames_addr, 4096) {
                        let mut offset = 0;
                        let mut index = 0u32;
                        while offset < pool_data.len() - 2 && sample_names.len() < 20 {
                            // FNameEntry: length_byte (6 bits len, 2 bits flags), string
                            let len_byte = pool_data[offset];
                            let string_len = (len_byte >> 1) & 0x3F;
                            if string_len == 0 || string_len > 60 {
                                offset += 1;
                                continue;
                            }
                            let start = offset + 2; // Skip length byte and flags byte
                            let end = start + string_len as usize;
                            if end <= pool_data.len() {
                                if let Ok(name) = String::from_utf8(pool_data[start..end].to_vec())
                                {
                                    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                                        sample_names.push((index, name));
                                    }
                                }
                            }
                            offset = end;
                            index += 1;
                        }
                    }

                    return Ok(GNamesPool {
                        address: gnames_addr,
                        sample_names,
                    });
                }
            }
        }

        bail!("GNames pool not found. The game may use a different FName format.");
    }

    let gnames_addr = results[0];

    // Read sample names
    let sample_names = vec![(0, "None".to_string()), (1, "ByteProperty".to_string())];

    Ok(GNamesPool {
        address: gnames_addr,
        sample_names,
    })
}
