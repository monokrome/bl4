//! GNames pool discovery
//!
//! Discovers the GNames pool using SDK offsets first (fast path),
//! falling back to brute-force pattern scanning if needed.

use super::super::binary::scan_pattern;
use super::super::fname::FNamePool;
use super::super::source::MemorySource;
use super::super::ue5::GNamesPool;

use anyhow::{bail, Result};

/// Discover GNames pool, trying SDK offsets first before pattern scanning.
pub fn discover_gnames(source: &dyn MemorySource) -> Result<GNamesPool> {
    // Fast path: use FNamePool::discover() which tries SDK constants first
    if let Ok(pool) = FNamePool::discover(source) {
        // Block 0 of FNamePool is the GNames data start
        if let Some(&block0) = pool.blocks.first() {
            if block0 != 0 {
                eprintln!("GNames discovered via FNamePool at block0={:#x}", block0);
                return Ok(GNamesPool {
                    address: block0,
                    sample_names: vec![
                        (0, "None".to_string()),
                        (1, "ByteProperty".to_string()),
                    ],
                });
            }
        }
    }

    // Slow path: brute-force pattern scan for "None" + "ByteProperty"
    eprintln!("FNamePool discovery failed, falling back to pattern scan...");
    discover_gnames_by_pattern(source)
}

/// Brute-force pattern scan for GNames (original approach)
fn discover_gnames_by_pattern(source: &dyn MemorySource) -> Result<GNamesPool> {
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
            if let Ok(data) = source.read_bytes(addr.saturating_sub(2), 64) {
                if data.windows(12).any(|w| w == b"ByteProperty") {
                    let gnames_addr = addr - 2;

                    let mut sample_names = Vec::new();
                    sample_names.push((0, "None".to_string()));
                    sample_names.push((1, "ByteProperty".to_string()));

                    if let Ok(pool_data) = source.read_bytes(gnames_addr, 4096) {
                        let mut offset = 0;
                        let mut index = 0u32;
                        while offset < pool_data.len() - 2 && sample_names.len() < 20 {
                            let len_byte = pool_data[offset];
                            let string_len = (len_byte >> 1) & 0x3F;
                            if string_len == 0 || string_len > 60 {
                                offset += 1;
                                continue;
                            }
                            let start = offset + 2;
                            let end = start + string_len as usize;
                            if end <= pool_data.len() {
                                if let Ok(name) =
                                    String::from_utf8(pool_data[start..end].to_vec())
                                {
                                    if name
                                        .chars()
                                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                                    {
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
    let sample_names = vec![(0, "None".to_string()), (1, "ByteProperty".to_string())];

    Ok(GNamesPool {
        address: gnames_addr,
        sample_names,
    })
}
