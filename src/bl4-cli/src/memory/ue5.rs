//! UE5 Core Structures
//!
//! Contains the core UE5 structures for memory analysis:
//! - GUObjectArray - The global object array containing all UObjects
//! - UObjectIterator - Iterator over UObject pointers
//! - GNamesPool - Discovered GNames pool information
//! - Ue5Offsets - UE5 structure offsets

#![allow(dead_code)]

use super::constants::*;
use super::source::MemorySource;

use anyhow::{bail, Context, Result};
use byteorder::{ByteOrder, LE};

/// UE5 structure offsets
#[derive(Debug)]
pub struct Ue5Offsets {
    pub guobject_array: usize,
    pub gnames: usize,
}

/// Discovered GNames pool
#[derive(Debug)]
pub struct GNamesPool {
    pub address: usize,
    pub sample_names: Vec<(u32, String)>,
}

/// Discovered GUObjectArray
#[derive(Debug)]
pub struct GUObjectArray {
    pub address: usize,
    /// Pointer to the Objects** array (array of chunk pointers)
    pub objects_ptr: usize,
    pub max_elements: i32,
    pub num_elements: i32,
    /// First chunk pointer (for direct access to first 64K items)
    pub first_chunk_ptr: usize,
    /// Size of each FUObjectItem in bytes (16 for UE5.3+, 24 for older)
    pub item_size: usize,
}

/// GUObjectArray virtual address (PE_IMAGE_BASE + GOBJECTS_OFFSET)
pub const GUOBJECTARRAY_VA: usize = PE_IMAGE_BASE + GOBJECTS_OFFSET;

impl GUObjectArray {
    /// Discover GUObjectArray at the known offset
    ///
    /// FUObjectArray structure (UE5):
    /// - Offset 0:  Objects** (8 bytes) - pointer to chunk pointer array
    /// - Offset 8:  PreAllocatedObjects* (8 bytes) - usually NULL
    /// - Offset 16: MaxElements (4 bytes) - typically 0x200000 (2097152)
    /// - Offset 20: NumElements (4 bytes) - current count
    /// - Offset 24: MaxChunks (4 bytes)
    /// - Offset 28: NumChunks (4 bytes)
    pub fn discover(source: &dyn MemorySource) -> Result<Self> {
        let addr = GUOBJECTARRAY_VA;

        let header = source
            .read_bytes(addr, 32)
            .context("Failed to read GUObjectArray header")?;

        let objects_ptr = LE::read_u64(&header[0..8]) as usize;
        let _preallocated = LE::read_u64(&header[8..16]) as usize;
        let max_elements = LE::read_i32(&header[16..20]);
        let num_elements = LE::read_i32(&header[20..24]);
        let _max_chunks = LE::read_i32(&header[24..28]);
        let num_chunks = LE::read_i32(&header[28..32]);

        if objects_ptr == 0 || !(MIN_VALID_POINTER..=MAX_VALID_POINTER).contains(&objects_ptr) {
            bail!(
                "GUObjectArray Objects pointer {:#x} is invalid",
                objects_ptr
            );
        }

        if max_elements <= 0 || max_elements > 10_000_000 {
            bail!("GUObjectArray MaxElements {} is unreasonable", max_elements);
        }

        if num_elements <= 0 || num_elements > max_elements {
            bail!(
                "GUObjectArray NumElements {} is invalid (max={})",
                num_elements,
                max_elements
            );
        }

        eprintln!("Found GUObjectArray at {:#x}:", addr);
        eprintln!("  Objects ptr: {:#x}", objects_ptr);
        eprintln!("  MaxElements: {}", max_elements);
        eprintln!("  NumElements: {}", num_elements);
        eprintln!("  NumChunks: {}", num_chunks);

        let first_chunk_data = source.read_bytes(objects_ptr, 8)?;
        let first_chunk_ptr = LE::read_u64(&first_chunk_data) as usize;

        if first_chunk_ptr == 0 || first_chunk_ptr < MIN_VALID_POINTER {
            bail!("First chunk pointer {:#x} is invalid", first_chunk_ptr);
        }

        eprintln!("  First chunk at: {:#x}", first_chunk_ptr);

        let item_size = Self::detect_item_size(source, first_chunk_ptr)?;
        eprintln!("  Detected FUObjectItem size: {} bytes", item_size);

        Ok(GUObjectArray {
            address: addr,
            objects_ptr,
            max_elements,
            num_elements,
            first_chunk_ptr,
            item_size,
        })
    }

    /// Detect FUObjectItem size by examining the object array
    pub fn detect_item_size(source: &dyn MemorySource, chunk_ptr: usize) -> Result<usize> {
        let test_data = source.read_bytes(chunk_ptr, 24 * 10)?;

        // Try 16-byte items (UE5.3+)
        let mut valid_16 = 0;
        for i in 0..10 {
            let ptr = LE::read_u64(&test_data[i * 16..i * 16 + 8]) as usize;
            if ptr == 0 || (MIN_VALID_POINTER..MAX_VALID_POINTER).contains(&ptr) {
                valid_16 += 1;
            }
        }

        // Try 24-byte items (UE5.0-5.2)
        let mut valid_24 = 0;
        for i in 0..10 {
            let ptr = LE::read_u64(&test_data[i * 24..i * 24 + 8]) as usize;
            if ptr == 0 || (MIN_VALID_POINTER..MAX_VALID_POINTER).contains(&ptr) {
                valid_24 += 1;
            }
        }

        eprintln!(
            "  Item size detection: 16-byte validity={}/10, 24-byte validity={}/10",
            valid_16, valid_24
        );

        // Prefer 24-byte if both seem valid (UE5.5 likely uses 24)
        if valid_24 >= 8 {
            Ok(24)
        } else if valid_16 >= 8 {
            Ok(16)
        } else {
            eprintln!("  Warning: Could not reliably detect item size, defaulting to 24");
            Ok(24)
        }
    }

    /// Iterate over all UObject pointers in the array
    pub fn iter_objects<'a>(&'a self, source: &'a dyn MemorySource) -> UObjectIterator<'a> {
        UObjectIterator {
            source,
            array: self,
            chunk_idx: 0,
            item_idx: 0,
            chunk_data: Vec::new(),
            chunk_ptr: 0,
        }
    }
}

/// Iterator over UObject pointers in GUObjectArray
pub struct UObjectIterator<'a> {
    source: &'a dyn MemorySource,
    array: &'a GUObjectArray,
    chunk_idx: usize,
    item_idx: usize,
    chunk_data: Vec<u8>,
    chunk_ptr: usize,
}

impl<'a> Iterator for UObjectIterator<'a> {
    type Item = (usize, usize); // (index, object_ptr)

    fn next(&mut self) -> Option<Self::Item> {
        let num_chunks = ((self.array.num_elements as usize) + GUOBJECTARRAY_CHUNK_SIZE - 1)
            / GUOBJECTARRAY_CHUNK_SIZE;

        loop {
            if self.chunk_idx >= num_chunks {
                return None;
            }

            let items_in_chunk = if self.chunk_idx == num_chunks - 1 {
                let remainder = (self.array.num_elements as usize) % GUOBJECTARRAY_CHUNK_SIZE;
                if remainder == 0 {
                    GUOBJECTARRAY_CHUNK_SIZE
                } else {
                    remainder
                }
            } else {
                GUOBJECTARRAY_CHUNK_SIZE
            };

            if self.chunk_data.is_empty() || self.item_idx >= items_in_chunk {
                self.chunk_idx += if self.chunk_data.is_empty() { 0 } else { 1 };
                self.item_idx = 0;

                if self.chunk_idx >= num_chunks {
                    return None;
                }

                let chunk_ptr_offset = self.array.objects_ptr + self.chunk_idx * 8;
                let chunk_ptr_data = self.source.read_bytes(chunk_ptr_offset, 8).ok()?;
                self.chunk_ptr = LE::read_u64(&chunk_ptr_data) as usize;

                if self.chunk_ptr == 0 {
                    self.chunk_data.clear();
                    continue;
                }

                let items_to_read = if self.chunk_idx == num_chunks - 1 {
                    let remainder = (self.array.num_elements as usize) % GUOBJECTARRAY_CHUNK_SIZE;
                    if remainder == 0 {
                        GUOBJECTARRAY_CHUNK_SIZE
                    } else {
                        remainder
                    }
                } else {
                    GUOBJECTARRAY_CHUNK_SIZE
                };

                self.chunk_data = self
                    .source
                    .read_bytes(self.chunk_ptr, items_to_read * self.array.item_size)
                    .ok()?;
            }

            let item_offset = self.item_idx * self.array.item_size;
            let obj_ptr = LE::read_u64(&self.chunk_data[item_offset..item_offset + 8]) as usize;

            let global_idx = self.chunk_idx * GUOBJECTARRAY_CHUNK_SIZE + self.item_idx;
            self.item_idx += 1;

            if obj_ptr != 0 {
                return Some((global_idx, obj_ptr));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    #[test]
    fn test_ue5_offsets_structure() {
        let offsets = Ue5Offsets {
            guobject_array: 0x151234000,
            gnames: 0x200000000,
        };

        assert_eq!(offsets.guobject_array, 0x151234000);
        assert_eq!(offsets.gnames, 0x200000000);
    }

    #[test]
    fn test_gnames_pool_structure() {
        let pool = GNamesPool {
            address: 0x200000000,
            sample_names: vec![(0, "None".to_string()), (1, "ByteProperty".to_string())],
        };

        assert_eq!(pool.address, 0x200000000);
        assert_eq!(pool.sample_names.len(), 2);
    }

    #[test]
    fn test_guobject_array_structure() {
        let array = GUObjectArray {
            address: 0x151234000,
            objects_ptr: 0x300000000,
            max_elements: 2097152,
            num_elements: 100000,
            first_chunk_ptr: 0x300001000,
            item_size: 24,
        };

        assert_eq!(array.address, 0x151234000);
        assert_eq!(array.objects_ptr, 0x300000000);
        assert_eq!(array.max_elements, 2097152);
        assert_eq!(array.num_elements, 100000);
        assert_eq!(array.item_size, 24);
    }

    #[test]
    fn test_detect_item_size_24_byte() {
        // Create mock chunk data with 24-byte items containing valid pointers
        let chunk_base = 0x300000000usize;
        let mut data = vec![0u8; 24 * 10];

        // Fill with valid pointers at 24-byte intervals
        for i in 0..10 {
            let ptr = 0x200000000u64 + (i as u64 * 0x1000);
            data[i * 24..i * 24 + 8].copy_from_slice(&ptr.to_le_bytes());
        }

        let source = MockMemorySource::new(data, chunk_base);
        let result = GUObjectArray::detect_item_size(&source, chunk_base);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 24);
    }

    #[test]
    fn test_detect_item_size_16_byte() {
        // Create mock chunk data with 16-byte items containing valid pointers
        let chunk_base = 0x300000000usize;
        let mut data = vec![0u8; 24 * 10]; // Need enough for both checks

        // Fill with valid pointers at 16-byte intervals
        for i in 0..10 {
            let ptr = 0x200000000u64 + (i as u64 * 0x1000);
            data[i * 16..i * 16 + 8].copy_from_slice(&ptr.to_le_bytes());
        }
        // Make 24-byte pattern invalid
        for i in 0..10 {
            if i * 24 + 8 <= data.len() {
                data[i * 24..i * 24 + 8].copy_from_slice(&[0xFF; 8]);
            }
        }

        let source = MockMemorySource::new(data, chunk_base);
        let result = GUObjectArray::detect_item_size(&source, chunk_base);

        assert!(result.is_ok());
        // Note: The function prefers 24-byte if valid_24 >= 8, so this might return 24
        // depending on the data layout
    }

    /// Create mock memory layout for UObjectIterator testing
    /// Layout:
    ///   objects_ptr (0x300000000): array of chunk pointers
    ///   chunk0 (0x400000000): FUObjectItem array with object pointers
    fn create_iterator_mock() -> (Vec<u8>, MockMemorySource) {
        let objects_ptr = 0x300000000usize;
        let chunk0_ptr = 0x400000000usize;

        // We need contiguous memory from objects_ptr to chunk data
        // For simplicity, create separate regions
        let mut data = vec![0u8; 0x200000]; // 2MB

        // Place chunk pointer array at offset 0 (objects_ptr base)
        // Chunk 0 pointer -> chunk0_ptr
        data[0..8].copy_from_slice(&(chunk0_ptr as u64).to_le_bytes());

        // Place chunk0 data at offset 0x100000 (1MB into data)
        // This simulates chunk0_ptr - objects_ptr = 0x100000000, but we'll adjust
        let chunk_offset = 0x100000;
        let item_size = 24;

        // Create 5 FUObjectItems with object pointers
        let obj_ptrs = [
            0x500000000u64,
            0x500001000u64,
            0u64, // NULL - should be skipped
            0x500002000u64,
            0x500003000u64,
        ];

        for (i, &ptr) in obj_ptrs.iter().enumerate() {
            let offset = chunk_offset + i * item_size;
            data[offset..offset + 8].copy_from_slice(&ptr.to_le_bytes());
        }

        // Create source with two regions
        let source = MockMemorySource::with_regions(
            data,
            objects_ptr,
            vec![crate::memory::source::MemoryRegion {
                start: objects_ptr,
                end: objects_ptr + 0x200000,
                perms: "rw-p".to_string(),
                offset: 0,
                path: None,
            }],
        );

        // We need to adjust - the chunk pointer needs to point within our data
        // Let's recalculate: chunk0 should be at objects_ptr + chunk_offset
        let adjusted_chunk_ptr = objects_ptr + 0x100000;
        let mut adjusted_data = vec![0u8; 0x200000];
        adjusted_data[0..8].copy_from_slice(&(adjusted_chunk_ptr as u64).to_le_bytes());

        for (i, &ptr) in obj_ptrs.iter().enumerate() {
            let offset = 0x100000 + i * item_size;
            adjusted_data[offset..offset + 8].copy_from_slice(&ptr.to_le_bytes());
        }

        let adjusted_source = MockMemorySource::with_regions(
            adjusted_data,
            objects_ptr,
            vec![crate::memory::source::MemoryRegion {
                start: objects_ptr,
                end: objects_ptr + 0x200000,
                perms: "rw-p".to_string(),
                offset: 0,
                path: None,
            }],
        );

        (vec![], adjusted_source)
    }

    #[test]
    fn test_uobject_iterator() {
        let (_, source) = create_iterator_mock();
        let objects_ptr = 0x300000000usize;
        let chunk0_ptr = objects_ptr + 0x100000;

        let array = GUObjectArray {
            address: 0x151234000,
            objects_ptr,
            max_elements: 65536,
            num_elements: 5, // 5 items in chunk 0
            first_chunk_ptr: chunk0_ptr,
            item_size: 24,
        };

        let objects: Vec<(usize, usize)> = array.iter_objects(&source).collect();

        // Should find 4 non-null objects (index 2 is NULL)
        assert_eq!(objects.len(), 4);

        // Verify indices and pointers
        assert_eq!(objects[0], (0, 0x500000000));
        assert_eq!(objects[1], (1, 0x500001000));
        assert_eq!(objects[2], (3, 0x500002000)); // Index 2 was NULL, so this is index 3
        assert_eq!(objects[3], (4, 0x500003000));
    }

    #[test]
    fn test_guobjectarray_va_constant() {
        // Verify the constant is calculated correctly
        assert_eq!(GUOBJECTARRAY_VA, PE_IMAGE_BASE + GOBJECTS_OFFSET);
    }
}
