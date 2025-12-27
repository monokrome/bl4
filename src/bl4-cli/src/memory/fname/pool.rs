//! FNamePool structure and discovery
//!
//! The FNamePool is UE5's block-based storage for FName strings.
//! Each block is typically 64KB.

use super::super::constants::*;
use super::super::source::MemorySource;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// FNamePool structure discovered in BL4 UE5.5
/// The pool header is at a fixed location, with blocks stored in an array
#[derive(Debug, Clone)]
pub struct FNamePool {
    /// Address of the FNamePool header
    pub header_addr: usize,
    /// Current block count
    pub current_block: u32,
    /// Current byte cursor in the current block
    pub current_cursor: u32,
    /// Cached block addresses
    pub blocks: Vec<usize>,
}

impl FNamePool {
    /// Discover the FNamePool dynamically by searching for header pointing to known GNames
    ///
    /// The FNamePool header layout (UE5.5):
    /// +0x00: Lock (8 bytes) - should be 0 or small value
    /// +0x08: CurrentBlock (4 bytes)
    /// +0x0C: CurrentByteCursor (4 bytes)
    /// +0x10: Blocks[] - array of block pointers (8 bytes each)
    pub fn discover(source: &dyn MemorySource) -> Result<Self> {
        // First try known SDK location
        if let Ok(pool) = Self::discover_at_address(source, FNAMEPOOL_HEADER_ADDR) {
            return Ok(pool);
        }

        eprintln!("SDK FNamePool location invalid, searching dynamically...");

        // Search for FNamePool header in data sections
        let mut regions_to_search: Vec<_> = source
            .regions()
            .iter()
            .filter(|r| r.is_readable())
            .collect();
        regions_to_search.sort_by_key(|r| {
            if r.start >= 0x140000000 && r.start < 0x160000000 {
                0 // PE sections first
            } else if r.start >= 0x1000000 && r.start < 0x140000000 {
                1 // Low heap regions second
            } else {
                2 // Everything else
            }
        });

        for region in regions_to_search {
            let data = match source.read_bytes(region.start, region.size().min(16 * 1024 * 1024)) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for i in (0..data.len().saturating_sub(32)).step_by(8) {
                let lock = LE::read_u64(&data[i..i + 8]);
                let current_block = LE::read_u32(&data[i + 8..i + 12]);
                let current_cursor = LE::read_u32(&data[i + 12..i + 16]);
                let block0 = LE::read_u64(&data[i + 16..i + 24]) as usize;

                // Validate pattern
                if lock > 100 {
                    continue;
                }
                if current_block == 0 || current_block > 1000 {
                    continue;
                }
                if current_cursor == 0 || current_cursor > 0x100000 {
                    continue;
                }
                if block0 < 0x100000 || block0 > 0x800000000000 || block0 % 8 != 0 {
                    continue;
                }

                // Validate Block0 contains "None" at offset 0
                if let Ok(entry_data) = source.read_bytes(block0, 64) {
                    let header0 = LE::read_u16(&entry_data[0..2]);
                    let len0 = (header0 >> 6) as usize;
                    if len0 == 4 && &entry_data[2..6] == b"None" {
                        let header_addr = region.start + i;
                        eprintln!(
                            "Found FNamePool at {:#x}: lock={}, blocks={}, cursor={}, block0={:#x}",
                            header_addr, lock, current_block, current_cursor, block0
                        );

                        // Read all block pointers
                        let num_blocks = (current_block + 1) as usize;
                        let blocks_data = source.read_bytes(header_addr + 16, num_blocks * 8)?;
                        let blocks: Vec<usize> = blocks_data
                            .chunks_exact(8)
                            .map(|c| LE::read_u64(c) as usize)
                            .collect();

                        return Ok(FNamePool {
                            header_addr,
                            current_block,
                            current_cursor,
                            blocks,
                        });
                    }
                }
            }
        }

        bail!("FNamePool header not found")
    }

    /// Try to discover FNamePool at a specific address
    fn discover_at_address(source: &dyn MemorySource, addr: usize) -> Result<Self> {
        let header_data = source.read_bytes(addr, 24)?;
        let lock = LE::read_u64(&header_data[0..8]);
        let current_block = LE::read_u32(&header_data[8..12]);
        let current_cursor = LE::read_u32(&header_data[12..16]);
        let block0 = LE::read_u64(&header_data[16..24]) as usize;

        // Validate header
        if current_block == 0 || current_block > 1000 {
            bail!("FNamePool current_block {} invalid", current_block);
        }
        if block0 == 0 || block0 < MIN_VALID_POINTER || block0 > MAX_VALID_POINTER {
            bail!("FNamePool block0 pointer {:#x} is invalid", block0);
        }

        // Verify block0 contains "None" at offset 0
        let entry_data = source.read_bytes(block0, 8)?;
        let header0 = LE::read_u16(&entry_data[0..2]);
        let len0 = (header0 >> 6) as usize;
        if len0 != 4 || &entry_data[2..6] != b"None" {
            bail!("Block0 doesn't start with 'None' entry");
        }

        eprintln!(
            "Found FNamePool at {:#x}: lock={}, blocks={}, cursor={}, block0={:#x}",
            addr, lock, current_block, current_cursor, block0
        );

        // Read all block pointers
        let num_blocks = (current_block + 1) as usize;
        let blocks_data = source.read_bytes(addr + 16, num_blocks * 8)?;
        let blocks: Vec<usize> = blocks_data
            .chunks_exact(8)
            .map(|c| LE::read_u64(c) as usize)
            .collect();

        Ok(FNamePool {
            header_addr: addr,
            current_block,
            current_cursor,
            blocks,
        })
    }

    /// Discover FNamePool using the known GNames pool address
    pub fn discover_with_gnames(source: &dyn MemorySource, gnames_addr: usize) -> Result<Self> {
        eprintln!(
            "Searching for FNamePool header with Block0 = {:#x}...",
            gnames_addr
        );

        let mut regions_to_search: Vec<_> = source
            .regions()
            .iter()
            .filter(|r| r.is_readable())
            .collect();

        regions_to_search.sort_by_key(|r| {
            if r.start >= 0x140000000 && r.start < 0x160000000 {
                0
            } else {
                1
            }
        });

        for region in regions_to_search {
            if !region.is_readable() {
                continue;
            }

            let data = match source.read_bytes(region.start, region.size().min(32 * 1024 * 1024)) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for i in (0..data.len().saturating_sub(32)).step_by(8) {
                let block0 = LE::read_u64(&data[i + 16..i + 24]) as usize;
                if block0 != gnames_addr {
                    continue;
                }

                let lock = LE::read_u64(&data[i..i + 8]);
                let current_block = LE::read_u32(&data[i + 8..i + 12]);
                let current_cursor = LE::read_u32(&data[i + 12..i + 16]);

                if lock > 100 || current_block == 0 || current_block > 1000 {
                    continue;
                }

                let header_addr = region.start + i;
                eprintln!(
                    "Found FNamePool at {:#x}: lock={}, blocks={}, cursor={}, block0={:#x}",
                    header_addr, lock, current_block, current_cursor, block0
                );

                let num_blocks = (current_block + 1) as usize;
                let blocks_data = source.read_bytes(header_addr + 16, num_blocks * 8)?;
                let blocks: Vec<usize> = blocks_data
                    .chunks_exact(8)
                    .map(|c| LE::read_u64(c) as usize)
                    .collect();

                return Ok(FNamePool {
                    header_addr,
                    current_block,
                    current_cursor,
                    blocks,
                });
            }
        }

        bail!("FNamePool header with Block0={:#x} not found", gnames_addr)
    }
}
