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
    /// Byte offset of Object* within each FUObjectItem (0 or 8)
    pub object_offset: usize,
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

        let (item_size, object_offset) = Self::detect_item_layout(source, first_chunk_ptr)?;

        Ok(GUObjectArray {
            address: addr,
            objects_ptr,
            max_elements,
            num_elements,
            first_chunk_ptr,
            item_size,
            object_offset,
        })
    }

    /// Detect FUObjectItem layout by examining the object array.
    ///
    /// Returns (item_size, object_offset) — tries all combinations of:
    /// - stride: 16 bytes (compact) or 24 bytes (standard UE5)
    /// - object_offset: 0 (Object* first) or 8 (FlagsAndRefCount first)
    ///
    /// Validates by reading the pointed-to addresses and checking for valid UObject
    /// vtable and ClassPrivate pointers.
    pub fn detect_item_layout(
        source: &dyn MemorySource,
        chunk_ptr: usize,
    ) -> Result<(usize, usize)> {
        // Read enough for 10 items at the largest stride (24) + max offset (8) + ptr (8)
        let test_data = source.read_bytes(chunk_ptr, 24 * 10 + 8 + 8)?;

        let layouts: &[(usize, usize)] = &[(24, 8), (16, 8), (24, 0), (16, 0)];
        let mut best_layout = (24, FUOBJECTITEM_OBJECT_OFFSET);
        let mut best_score = 0u32;

        for &(stride, obj_off) in layouts {
            let mut score = 0u32;
            for i in 0..10 {
                let off = i * stride + obj_off;
                if off + 8 > test_data.len() {
                    break;
                }
                let ptr = LE::read_u64(&test_data[off..off + 8]) as usize;
                if ptr == 0 {
                    score += 1; // null is acceptable
                    continue;
                }
                if !(MIN_VALID_POINTER..MAX_VALID_POINTER).contains(&ptr) {
                    continue;
                }
                // Deep check: verify the pointed-to address looks like a UObject
                match source.read_bytes(ptr, UOBJECT_HEADER_SIZE) {
                    Ok(uobj) => {
                        let vtable =
                            LE::read_u64(&uobj[UOBJECT_VTABLE_OFFSET..UOBJECT_VTABLE_OFFSET + 8])
                                as usize;
                        let class_ptr =
                            LE::read_u64(&uobj[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8])
                                as usize;
                        if (MIN_VTABLE_ADDR..=MAX_VTABLE_ADDR).contains(&vtable)
                            && (MIN_VALID_POINTER..MAX_VALID_POINTER).contains(&class_ptr)
                        {
                            score += 2; // confirmed UObject
                        } else {
                            score += 1; // valid pointer, not confirmed UObject
                        }
                    }
                    Err(_) => {
                        score += 1; // valid pointer but target unreadable
                    }
                }
            }

            eprintln!(
                "  Layout (stride={}, offset={}): score={}/20",
                stride, obj_off, score
            );

            if score > best_score {
                best_score = score;
                best_layout = (stride, obj_off);
            }
        }

        if best_score < 8 {
            eprintln!(
                "  Warning: low confidence layout detection (score={}), using {:?}",
                best_score, best_layout
            );
        }

        eprintln!(
            "  Detected layout: stride={}, object_offset={}",
            best_layout.0, best_layout.1
        );

        Ok(best_layout)
    }

    /// Detect FUObjectItem size (backward compatibility wrapper).
    pub fn detect_item_size(source: &dyn MemorySource, chunk_ptr: usize) -> Result<usize> {
        let (stride, _) = Self::detect_item_layout(source, chunk_ptr)?;
        Ok(stride)
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
        let num_chunks = (self.array.num_elements as usize).div_ceil(GUOBJECTARRAY_CHUNK_SIZE);

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

                // Read chunk pointer — skip chunk on failure instead of terminating
                let chunk_ptr_offset = self.array.objects_ptr + self.chunk_idx * 8;
                let chunk_ptr_data = match self.source.read_bytes(chunk_ptr_offset, 8) {
                    Ok(d) => d,
                    Err(_) => {
                        self.chunk_data = vec![0]; // non-empty sentinel to advance chunk_idx
                        self.item_idx = items_in_chunk; // force next chunk
                        continue;
                    }
                };
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

                // Read chunk data — skip chunk on failure instead of terminating
                self.chunk_data = match self
                    .source
                    .read_bytes(self.chunk_ptr, items_to_read * self.array.item_size)
                {
                    Ok(d) => d,
                    Err(_) => {
                        self.chunk_data = vec![0]; // non-empty sentinel
                        self.item_idx = items_in_chunk; // force next chunk
                        continue;
                    }
                };
            }

            let item_offset = self.item_idx * self.array.item_size;
            let ptr_offset = item_offset + self.array.object_offset;

            // Bounds check — if chunk was partially read, skip remaining items
            if ptr_offset + 8 > self.chunk_data.len() {
                self.item_idx = items_in_chunk; // force next chunk
                continue;
            }

            let obj_ptr = LE::read_u64(&self.chunk_data[ptr_offset..ptr_offset + 8]) as usize;

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
            object_offset: FUOBJECTITEM_OBJECT_OFFSET,
        };

        assert_eq!(array.address, 0x151234000);
        assert_eq!(array.objects_ptr, 0x300000000);
        assert_eq!(array.max_elements, 2097152);
        assert_eq!(array.num_elements, 100000);
        assert_eq!(array.item_size, 24);
        assert_eq!(array.object_offset, 8);
    }

    #[test]
    fn test_detect_item_size_24_byte() {
        // Create mock chunk data with 24-byte items
        // FUObjectItem layout: FlagsAndRefCount(+0x00), Object*(+0x08), SerialNumber(+0x10)
        let chunk_base = 0x300000000usize;
        let mut data = vec![0u8; 24 * 10 + 16]; // extra for layout detection

        // Place valid Object* pointers at offset +8 within each 24-byte item
        for i in 0..10 {
            let flags = 0x0000000400000001u64; // Typical FlagsAndRefCount
            data[i * 24..i * 24 + 8].copy_from_slice(&flags.to_le_bytes());
            let ptr = 0x200000000u64 + (i as u64 * 0x1000);
            data[i * 24 + 8..i * 24 + 16].copy_from_slice(&ptr.to_le_bytes());
        }

        let source = MockMemorySource::new(data, chunk_base);
        let result = GUObjectArray::detect_item_size(&source, chunk_base);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 24);
    }

    #[test]
    fn test_detect_item_size_16_byte() {
        let chunk_base = 0x300000000usize;
        // Fill entirely with invalid pointer bytes, then place valid pointers
        // at offset +8 within each 16-byte item
        let mut data = vec![0xFFu8; 24 * 10 + 16]; // extra for layout detection

        for i in 0..10 {
            if i * 16 + 16 > data.len() {
                break;
            }
            // FlagsAndRefCount at +0
            let flags = 0x0000000400000001u64;
            data[i * 16..i * 16 + 8].copy_from_slice(&flags.to_le_bytes());
            // Object* at +8
            let ptr = 0x200000000u64 + (i as u64 * 0x1000);
            data[i * 16 + 8..i * 16 + 16].copy_from_slice(&ptr.to_le_bytes());
        }

        let source = MockMemorySource::new(data, chunk_base);
        let result = GUObjectArray::detect_item_size(&source, chunk_base);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 16);
    }

    /// Create mock memory layout for UObjectIterator testing
    /// Layout:
    ///   objects_ptr (0x300000000): array of chunk pointers
    ///   chunk0 (objects_ptr + 0x100000): FUObjectItem array
    ///
    /// FUObjectItem layout (24 bytes each):
    ///   +0x00: FlagsAndRefCount (8 bytes)
    ///   +0x08: Object* (8 bytes) - the UObject pointer
    ///   +0x10: SerialNumber + ClusterRootIndex (8 bytes)
    fn create_iterator_mock() -> (Vec<u8>, MockMemorySource) {
        let objects_ptr = 0x300000000usize;
        let item_size = 24;

        let adjusted_chunk_ptr = objects_ptr + 0x100000;
        let mut adjusted_data = vec![0u8; 0x200000];
        adjusted_data[0..8].copy_from_slice(&(adjusted_chunk_ptr as u64).to_le_bytes());

        // Create 5 FUObjectItems with Object* at offset +8 within each item
        let obj_ptrs = [
            0x500000000u64,
            0x500001000u64,
            0u64, // NULL - should be skipped
            0x500002000u64,
            0x500003000u64,
        ];

        for (i, &ptr) in obj_ptrs.iter().enumerate() {
            let base = 0x100000 + i * item_size;
            // FlagsAndRefCount at +0
            let flags = if ptr != 0 { 0x0000000400000001u64 } else { 0u64 };
            adjusted_data[base..base + 8].copy_from_slice(&flags.to_le_bytes());
            // Object* at +FUOBJECTITEM_OBJECT_OFFSET (0x08)
            adjusted_data[base + FUOBJECTITEM_OBJECT_OFFSET..base + FUOBJECTITEM_OBJECT_OFFSET + 8]
                .copy_from_slice(&ptr.to_le_bytes());
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
            object_offset: FUOBJECTITEM_OBJECT_OFFSET,
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
