//! FNameReader for reading FName strings from the pool
//!
//! Provides cached reading of FName entries with search capability.

use super::pool::FNamePool;
use super::super::constants::*;
use super::super::source::MemorySource;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// FNamePool reader for UE5
/// UE5 uses a chunked FNamePool with block-based storage
pub struct FNameReader {
    /// The FNamePool structure
    pub pool: FNamePool,
    /// Cached name entries: index -> name
    cache: std::collections::HashMap<u32, String>,
}

impl FNameReader {
    pub fn new(pool: FNamePool) -> Self {
        Self {
            pool,
            cache: std::collections::HashMap::new(),
        }
    }

    /// Legacy constructor for compatibility
    pub fn new_legacy(pool_base: usize) -> Self {
        Self {
            pool: FNamePool {
                header_addr: 0,
                current_block: 0,
                current_cursor: 0,
                blocks: vec![pool_base],
            },
            cache: std::collections::HashMap::new(),
        }
    }

    /// Read an FName entry from the pool
    /// FName index encoding in UE5:
    /// - ComparisonIndex = (BlockIndex << 16) | (BlockOffset >> 1)
    pub fn read_name(&mut self, source: &dyn MemorySource, fname_index: u32) -> Result<String> {
        if fname_index == 0 {
            return Ok("None".to_string());
        }

        if let Some(name) = self.cache.get(&fname_index) {
            return Ok(name.clone());
        }

        let comparison_index = fname_index & 0x3FFFFFFF;
        let block_index = (comparison_index >> 16) as usize;
        let block_offset = ((comparison_index & 0xFFFF) * 2) as usize;

        let block_addr = if block_index < self.pool.blocks.len() {
            self.pool.blocks[block_index]
        } else {
            bail!(
                "FName block {} out of range (have {} blocks)",
                block_index,
                self.pool.blocks.len()
            );
        };

        if block_addr == 0 {
            bail!("FName block {} is null", block_index);
        }

        let entry_addr = block_addr + block_offset;
        let header = source.read_bytes(entry_addr, 2)?;
        let header_val = LE::read_u16(&header);

        let is_wide = (header_val & 1) != 0;
        let len = (header_val >> 6) as usize;

        if len == 0 || len > 1024 {
            // Try alternative BL4-specific format
            let alt_len = ((header[0] >> 1) & 0x3F) as usize;
            if alt_len > 0 && alt_len <= 63 {
                let bytes = source.read_bytes(entry_addr + 2, alt_len)?;
                let name = String::from_utf8_lossy(&bytes).to_string();
                self.cache.insert(fname_index, name.clone());
                return Ok(name);
            }
            bail!(
                "Invalid FName length {} at index {} (block={}, offset={:#x}, header={:#x})",
                len,
                fname_index,
                block_index,
                block_offset,
                header_val
            );
        }

        let name = if is_wide {
            let bytes = source.read_bytes(entry_addr + 2, len * 2)?;
            let chars: Vec<u16> = bytes.chunks_exact(2).map(|c| LE::read_u16(c)).collect();
            String::from_utf16_lossy(&chars)
        } else {
            let bytes = source.read_bytes(entry_addr + 2, len)?;
            String::from_utf8_lossy(&bytes).to_string()
        };

        self.cache.insert(fname_index, name.clone());
        Ok(name)
    }

    /// Debug: dump information about an FName index
    pub fn debug_read(&self, source: &dyn MemorySource, fname_index: u32) -> Result<()> {
        let comparison_index = fname_index & 0x3FFFFFFF;
        let block_index = (comparison_index >> 16) as usize;
        let block_offset = ((comparison_index & 0xFFFF) * 2) as usize;

        eprintln!(
            "FName {} -> block={}, offset={:#x}",
            fname_index, block_index, block_offset
        );

        if block_index >= self.pool.blocks.len() {
            eprintln!("  Block out of range!");
            return Ok(());
        }

        let block_addr = self.pool.blocks[block_index];
        let entry_addr = block_addr + block_offset;
        eprintln!(
            "  Block addr: {:#x}, Entry addr: {:#x}",
            block_addr, entry_addr
        );

        let data = source.read_bytes(entry_addr, 32)?;
        eprint!("  Data: ");
        for b in &data {
            eprint!("{:02x} ", b);
        }
        eprintln!();

        eprint!("  ASCII: ");
        for b in &data {
            let c = *b as char;
            if c.is_ascii_graphic() || c == ' ' {
                eprint!("{}", c);
            } else {
                eprint!(".");
            }
        }
        eprintln!();

        Ok(())
    }

    /// Search for a string in the FNamePool and return its index
    pub fn search_name(&mut self, source: &dyn MemorySource, target: &str) -> Result<Option<u32>> {
        for (block_idx, &block_addr) in self.pool.blocks.iter().enumerate() {
            if block_addr == 0 {
                continue;
            }

            let block_size = 64 * 1024;
            let data = match source.read_bytes(block_addr, block_size) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let mut offset = 0usize;
            while offset + 2 < data.len() {
                let header_val = LE::read_u16(&data[offset..offset + 2]);
                let is_wide = (header_val & 1) != 0;
                let len = (header_val >> 6) as usize;

                if len == 0 || len > 1024 || offset + 2 + len > data.len() {
                    break;
                }

                let name = if is_wide {
                    let end = (offset + 2 + len * 2).min(data.len());
                    let chars: Vec<u16> = data[offset + 2..end]
                        .chunks_exact(2)
                        .map(|c| LE::read_u16(c))
                        .collect();
                    String::from_utf16_lossy(&chars)
                } else {
                    String::from_utf8_lossy(&data[offset + 2..offset + 2 + len]).to_string()
                };

                let fname_index = ((block_idx as u32) << 16) | ((offset as u32) / 2);
                self.cache.insert(fname_index, name.clone());

                if name == target {
                    return Ok(Some(fname_index));
                }

                let entry_size = 2 + if is_wide { len * 2 } else { len };
                offset += (entry_size + 1) & !1;
            }
        }

        Ok(None)
    }

    /// Find "Class" FName index dynamically
    pub fn find_class_index(&mut self, source: &dyn MemorySource) -> Result<u32> {
        if let Ok(name) = self.read_name(source, FNAME_CLASS_INDEX) {
            if name == "Class" {
                return Ok(FNAME_CLASS_INDEX);
            }
        }

        if let Some(idx) = self.search_name(source, "Class")? {
            eprintln!(
                "Found 'Class' FName at index {} (SDK said {})",
                idx, FNAME_CLASS_INDEX
            );
            return Ok(idx);
        }

        bail!("Could not find 'Class' FName in pool")
    }

    /// Find "Object" FName index dynamically
    pub fn find_object_index(&mut self, source: &dyn MemorySource) -> Result<u32> {
        if let Ok(name) = self.read_name(source, FNAME_OBJECT_INDEX) {
            if name == "Object" {
                return Ok(FNAME_OBJECT_INDEX);
            }
        }

        if let Some(idx) = self.search_name(source, "Object")? {
            eprintln!(
                "Found 'Object' FName at index {} (SDK said {})",
                idx, FNAME_OBJECT_INDEX
            );
            return Ok(idx);
        }

        bail!("Could not find 'Object' FName in pool")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    /// Create a mock FNamePool with test entries (contiguous layout for search_name)
    /// Block layout:
    ///   Offset 0: "None" (2+4=6 bytes, padded to 6) -> index 0
    ///   Offset 6: "ByteProperty" (2+12=14, padded to 14) -> index 3
    ///   Offset 20: "Class" (2+5=7, padded to 8) -> index 10
    ///   Offset 28: "Object" (2+6=8) -> index 14
    fn create_mock_fname_block() -> Vec<u8> {
        // Must be 64KB for search_name to work (it reads 64KB blocks)
        let mut block = vec![0u8; 64 * 1024];

        // Entry 0: "None" (length 4) at offset 0
        // Header: (len << 6) | flags = (4 << 6) | 0 = 0x100
        block[0] = 0x00; // Low byte of header
        block[1] = 0x01; // High byte (4 << 6 = 256 = 0x100)
        block[2..6].copy_from_slice(b"None");
        // entry_size = 2+4=6, next offset = (6+1)&!1 = 6

        // Entry 1: "ByteProperty" (length 12) at offset 6
        // Header: (12 << 6) = 0x300 = 768
        block[6] = 0x00;
        block[7] = 0x03;
        block[8..20].copy_from_slice(b"ByteProperty");
        // entry_size = 2+12=14, next offset = 6 + (14+1)&!1 = 6 + 14 = 20

        // Entry 2: "Class" (length 5) at offset 20
        // Header: (5 << 6) = 0x140 = 320
        block[20] = 0x40;
        block[21] = 0x01;
        block[22..27].copy_from_slice(b"Class");
        // entry_size = 2+5=7, next offset = 20 + (7+1)&!1 = 20 + 8 = 28

        // Entry 3: "Object" (length 6) at offset 28
        // Header: (6 << 6) = 0x180 = 384
        block[28] = 0x80;
        block[29] = 0x01;
        block[30..36].copy_from_slice(b"Object");

        block
    }

    /// Create a small mock block (1KB) for tests that don't use search_name
    fn create_small_fname_block() -> Vec<u8> {
        let mut block = vec![0u8; 1024];

        // Same layout as above but for direct index access
        block[0] = 0x00;
        block[1] = 0x01;
        block[2..6].copy_from_slice(b"None");

        block[6] = 0x00;
        block[7] = 0x03;
        block[8..20].copy_from_slice(b"ByteProperty");

        block[20] = 0x40;
        block[21] = 0x01;
        block[22..27].copy_from_slice(b"Class");

        block[28] = 0x80;
        block[29] = 0x01;
        block[30..36].copy_from_slice(b"Object");

        block
    }

    #[test]
    fn test_fname_reader_read_none() {
        let block = create_mock_fname_block();
        let block_addr = 0x200000000usize;

        let source = MockMemorySource::new(block, block_addr);

        let pool = FNamePool {
            header_addr: 0,
            current_block: 0,
            current_cursor: 100,
            blocks: vec![block_addr],
        };

        let mut reader = FNameReader::new(pool);

        // Index 0 should return "None" directly
        let name = reader.read_name(&source, 0).unwrap();
        assert_eq!(name, "None");
    }

    #[test]
    fn test_fname_reader_read_by_index() {
        let block = create_small_fname_block();
        let block_addr = 0x200000000usize;

        let source = MockMemorySource::new(block, block_addr);

        let pool = FNamePool {
            header_addr: 0,
            current_block: 0,
            current_cursor: 100,
            blocks: vec![block_addr],
        };

        let mut reader = FNameReader::new(pool);

        // Read "ByteProperty" at offset 6 (index = 6/2 = 3)
        let name = reader.read_name(&source, 3).unwrap();
        assert_eq!(name, "ByteProperty");

        // Read "Class" at offset 20 (index = 20/2 = 10)
        let name = reader.read_name(&source, 10).unwrap();
        assert_eq!(name, "Class");

        // Read "Object" at offset 28 (index = 28/2 = 14)
        let name = reader.read_name(&source, 14).unwrap();
        assert_eq!(name, "Object");
    }

    #[test]
    fn test_fname_reader_caches_results() {
        let block = create_small_fname_block();
        let block_addr = 0x200000000usize;

        let source = MockMemorySource::new(block, block_addr);

        let pool = FNamePool {
            header_addr: 0,
            current_block: 0,
            current_cursor: 100,
            blocks: vec![block_addr],
        };

        let mut reader = FNameReader::new(pool);

        // First read "Class" at index 10
        let name1 = reader.read_name(&source, 10).unwrap();
        assert_eq!(name1, "Class");

        // Second read should use cache
        let name2 = reader.read_name(&source, 10).unwrap();
        assert_eq!(name2, "Class");

        // Verify it's in cache
        assert!(reader.cache.contains_key(&10));
    }

    #[test]
    fn test_fname_reader_search_name() {
        let block = create_mock_fname_block();
        let block_addr = 0x200000000usize;

        let source = MockMemorySource::new(block, block_addr);

        let pool = FNamePool {
            header_addr: 0,
            current_block: 0,
            current_cursor: 100,
            blocks: vec![block_addr],
        };

        let mut reader = FNameReader::new(pool);

        // Search for "None" at offset 0 (index = 0)
        let idx = reader.search_name(&source, "None").unwrap();
        assert!(idx.is_some());
        assert_eq!(idx.unwrap(), 0);

        // Search for "ByteProperty" at offset 6 (index = 3)
        let idx = reader.search_name(&source, "ByteProperty").unwrap();
        assert!(idx.is_some());
        assert_eq!(idx.unwrap(), 3);

        // Search for "Class" at offset 20 (index = 10)
        let idx = reader.search_name(&source, "Class").unwrap();
        assert!(idx.is_some());
        assert_eq!(idx.unwrap(), 10);

        // Search for non-existent name
        let idx = reader.search_name(&source, "NotFound").unwrap();
        assert!(idx.is_none());
    }

    #[test]
    fn test_fname_reader_block_out_of_range() {
        let block = create_mock_fname_block();
        let block_addr = 0x200000000usize;

        let source = MockMemorySource::new(block, block_addr);

        let pool = FNamePool {
            header_addr: 0,
            current_block: 0,
            current_cursor: 100,
            blocks: vec![block_addr], // Only one block
        };

        let mut reader = FNameReader::new(pool);

        // Try to read from block 1 (which doesn't exist)
        // Index with block 1 = (1 << 16) | 0 = 0x10000
        let result = reader.read_name(&source, 0x10000);
        assert!(result.is_err());
    }

    #[test]
    fn test_fname_pool_structure() {
        let pool = FNamePool {
            header_addr: 0x151000000,
            current_block: 5,
            current_cursor: 32768,
            blocks: vec![0x200000000, 0x200010000, 0x200020000],
        };

        assert_eq!(pool.header_addr, 0x151000000);
        assert_eq!(pool.current_block, 5);
        assert_eq!(pool.blocks.len(), 3);
    }

    #[test]
    fn test_fname_reader_new_legacy() {
        let pool_base = 0x200000000;
        let reader = FNameReader::new_legacy(pool_base);

        assert_eq!(reader.pool.blocks.len(), 1);
        assert_eq!(reader.pool.blocks[0], pool_base);
        assert!(reader.cache.is_empty());
    }

    #[test]
    fn test_fname_index_encoding() {
        // FName index encoding: (block << 16) | (offset / 2)
        // Block 0, offset 100 -> index = 0 | 50 = 50
        let fname_index = 50u32;
        let comparison_index = fname_index & 0x3FFFFFFF;
        let block_index = (comparison_index >> 16) as usize;
        let block_offset = ((comparison_index & 0xFFFF) * 2) as usize;

        assert_eq!(block_index, 0);
        assert_eq!(block_offset, 100);

        // Block 1, offset 200 -> index = (1 << 16) | 100 = 65636
        let fname_index = (1 << 16) | 100;
        let comparison_index = fname_index & 0x3FFFFFFF;
        let block_index = (comparison_index >> 16) as usize;
        let block_offset = ((comparison_index & 0xFFFF) * 2) as usize;

        assert_eq!(block_index, 1);
        assert_eq!(block_offset, 200);
    }
}
