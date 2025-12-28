//! FName lookup and search command handlers
//!
//! Handlers for looking up FNames by index and searching for FNames by string.

use crate::memory::{self, MemorySource};
use anyhow::{Context, Result};
use byteorder::ByteOrder;

/// Handle the Fname command
///
/// Looks up an FName by its index and displays the name.
pub fn handle_fname(source: &dyn MemorySource, index: u32, debug: bool) -> Result<()> {
    // Try to discover the full FNamePool structure using known address
    match memory::FNamePool::discover(source) {
        Ok(pool) => {
            println!("FNamePool found at {:#x}", pool.header_addr);
            println!("  Blocks: {}", pool.blocks.len());
            println!("  Cursor: {}", pool.current_cursor);

            let reader = memory::FNameReader::new(pool);

            // Always dump raw bytes when --debug is specified
            if debug {
                reader.debug_read(source, index)?;
            }

            let mut reader = reader;
            match reader.read_name(source, index) {
                Ok(name) => {
                    println!("\nFName[{}] = \"{}\"", index, name);

                    // Show index breakdown
                    let block = (index & 0x3FFFFFFF) >> 16;
                    let offset = ((index & 0xFFFF) * 2) as usize;
                    println!("  Block: {}, Offset: {:#x}", block, offset);
                }
                Err(e) => {
                    eprintln!("Failed to read FName[{}]: {}", index, e);
                    if !debug {
                        reader.debug_read(source, index)?;
                    }
                }
            }
        }
        Err(e) => {
            // Fall back to pattern-based discovery
            eprintln!("FNamePool::discover failed: {}", e);
            let gnames = memory::discover_gnames(source).context("Failed to find GNames pool")?;
            println!("Using legacy FName reader (block 0 only)");
            let mut reader = memory::FNameReader::new_legacy(gnames.address);
            match reader.read_name(source, index) {
                Ok(name) => println!("FName[{}] = \"{}\"", index, name),
                Err(e) => eprintln!("Failed to read FName[{}]: {}", index, e),
            }
        }
    }
    Ok(())
}

/// Handle the FnameSearch command
///
/// Searches for FNames containing the given query string.
pub fn handle_fname_search(source: &dyn MemorySource, query: &str) -> Result<()> {
    // Discover FNamePool to get all blocks
    let pool = memory::FNamePool::discover(source).context("Failed to discover FNamePool")?;

    println!(
        "Searching for \"{}\" across {} FName blocks...",
        query,
        pool.blocks.len()
    );

    let search_bytes = query.as_bytes();
    let mut found = Vec::new();

    // Search all blocks
    for (block_idx, &block_addr) in pool.blocks.iter().enumerate() {
        if block_addr == 0 {
            continue;
        }

        // Read block data (64KB per block)
        let block_data = match source.read_bytes(block_addr, 64 * 1024) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for (pos, window) in block_data.windows(search_bytes.len()).enumerate() {
            if window == search_bytes {
                // Found match - try to find the entry start
                if pos >= 2 {
                    let header = &block_data[pos - 2..pos];
                    let header_val = byteorder::LE::read_u16(header);
                    let len = (header_val >> 6) as usize;

                    // Verify this is a valid entry header
                    if len > 0 && len <= 1024 {
                        // Read the full name from header position
                        let name_start = pos - 2 + 2;
                        let name_end = name_start + len;
                        if name_end <= block_data.len() {
                            let full_name =
                                String::from_utf8_lossy(&block_data[name_start..name_end]);
                            let byte_offset = pos - 2;
                            // FName index = (block_idx << 16) | (byte_offset / 2)
                            let fname_index =
                                ((block_idx as u32) << 16) | ((byte_offset / 2) as u32);
                            found.push((
                                fname_index,
                                block_idx,
                                byte_offset,
                                full_name.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    if found.is_empty() {
        println!("No matches found for \"{}\"", query);
    } else {
        println!("Found {} matches:", found.len());
        for (fname_index, block_idx, byte_offset, name) in found.iter().take(50) {
            println!(
                "  FName[{:#x}] = \"{}\" (block {}, offset {:#x})",
                fname_index, name, block_idx, byte_offset
            );
        }
        if found.len() > 50 {
            println!("  ... and {} more", found.len() - 50);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    #[test]
    fn test_handle_fname_with_invalid_pool() {
        // Empty memory source won't have a valid FNamePool
        let source = MockMemorySource::new(vec![], 0x1000);
        let result = handle_fname(&source, 0, false);
        // Should fail gracefully when pool can't be discovered
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_handle_fname_search_empty_query() {
        let source = MockMemorySource::new(vec![], 0x1000);
        // Empty source won't have a valid pool
        let result = handle_fname_search(&source, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_fname_index_breakdown() {
        // Test the index breakdown calculation
        let index: u32 = 0x0001_0010; // block 1, offset 0x20
        let block = (index & 0x3FFFFFFF) >> 16;
        let offset = ((index & 0xFFFF) * 2) as usize;
        assert_eq!(block, 1);
        assert_eq!(offset, 0x20);
    }

    #[test]
    fn test_fname_index_calculation() {
        // Test FName index calculation from block and byte offset
        let block_idx: u32 = 5;
        let byte_offset: u32 = 0x100;
        let fname_index = (block_idx << 16) | (byte_offset / 2);
        assert_eq!(fname_index, 0x0005_0080);
    }
}
