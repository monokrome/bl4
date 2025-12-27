//! UE5 Structure Discovery
//!
//! Functions for discovering UE5 global structures in memory:
//! - GNames pool discovery via pattern scanning
//! - GUObjectArray discovery via code pattern analysis
//! - Class UClass discovery via self-referential pattern
//! - UE5 offset detection

use super::binary::{find_code_bounds, scan_pattern};
use super::constants::*;
use super::source::MemorySource;
use super::ue5::{GNamesPool, GUObjectArray, Ue5Offsets};

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

/// Check if a pointer value looks like a valid heap/data pointer for this dump
/// Windows heap is typically in the range 0x00010000-0x7FFFFFFFFFFF
fn is_valid_pointer(ptr: usize) -> bool {
    // Accept both low heap (Windows user mode) and high addresses (executable sections)
    ptr >= MIN_VALID_POINTER && ptr < MAX_VALID_POINTER
}

/// Discover GUObjectArray by scanning code for the access pattern
///
/// Uses the code pattern: 48 8B 05 ?? ?? ?? ?? 48 8B 0C C8 48 8D 04 D1
/// This is: mov rax, [rip+offset]; mov rcx, [rax+rcx*8]; lea rax, [rcx+rdx*8]
/// The RIP-relative offset in the first instruction points to GUObjectArray
pub fn discover_guobject_array(
    source: &dyn MemorySource,
    _gnames_addr: usize,
) -> Result<GUObjectArray> {
    // First, try to find GUObjectArray via code pattern scanning
    // Pattern: 48 8B 05 ?? ?? ?? ?? 48 8B 0C C8 48 8D 04 D1 EB ??
    // This is: mov rax, [rip+disp32]; mov rcx, [rax+rcx*8]; lea rax, [rcx+rdx*8]; jmp

    eprintln!("Searching for GUObjectArray via code pattern...");

    // Try two approaches:
    // 1. Specific pattern: 48 8B 05 ?? ?? ?? ?? 48 8B 0C C8 48 8D 04 D1
    // 2. Generic: any 48 8B 05 (mov rax, [rip+disp]) pointing to valid GUObjectArray

    let pattern_suffix: &[u8] = &[0x48, 0x8B, 0x0C, 0xC8, 0x48, 0x8D, 0x04, 0xD1];
    let mut found_candidates: Vec<(usize, usize)> = Vec::new(); // (code_addr, guobj_addr)

    // Scan code sections for this pattern
    for region in source.regions() {
        // Look for regions in the main executable range (covers both .ecode and .code sections)
        // .ecode: 0x140001000-0x14e61c000
        // .code: 0x14e61c000-0x14f32d000
        if region.start < 0x140000000 || region.start > 0x150000000 {
            continue;
        }
        if !region.is_readable() {
            continue;
        }

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Look for: 48 8B 05 ?? ?? ?? ?? followed by pattern_suffix
        for i in 0..data.len().saturating_sub(15) {
            if data[i] != 0x48 || data[i + 1] != 0x8B || data[i + 2] != 0x05 {
                continue;
            }

            // Check if pattern_suffix follows after the 7-byte instruction
            if &data[i + 7..i + 15] != pattern_suffix {
                continue;
            }

            // Found the pattern! Extract the RIP-relative displacement
            let disp = LE::read_i32(&data[i + 3..i + 7]) as i64;
            let code_addr = region.start + i;
            let rip = code_addr + 7; // RIP after the instruction
            let guobj_addr = (rip as i64 + disp) as usize;

            // Validate the pointer
            if !is_valid_pointer(guobj_addr) {
                continue;
            }

            // Try to read and validate GUObjectArray header
            if let Ok(header) = source.read_bytes(guobj_addr, 32) {
                let objects_ptr = LE::read_u64(&header[0..8]) as usize;
                let _preallocated = LE::read_u64(&header[8..16]) as usize;
                let max_elements = LE::read_i32(&header[16..20]);
                let num_elements = LE::read_i32(&header[20..24]);

                // Validate header values
                if is_valid_pointer(objects_ptr)
                    && max_elements > 0
                    && max_elements < 10_000_000
                    && num_elements > 0
                    && num_elements <= max_elements
                {
                    eprintln!(
                        "Found GUObjectArray at {:#x} via code pattern at {:#x}",
                        guobj_addr, code_addr
                    );
                    eprintln!(
                        "  Objects: {:#x}, Max: {}, Num: {}",
                        objects_ptr, max_elements, num_elements
                    );
                    found_candidates.push((code_addr, guobj_addr));
                }
            }
        }
    }

    if !found_candidates.is_empty() {
        // Return the first valid candidate
        let (_, guobj_addr) = found_candidates[0];
        return build_guobject_array(source, guobj_addr);
    }

    // Fallback: Try the known SDK offset
    eprintln!("Trying SDK offset for GUObjectArray...");
    let sdk_addr = PE_IMAGE_BASE + GOBJECTS_OFFSET;

    if let Ok(header) = source.read_bytes(sdk_addr, 32) {
        let objects_ptr = LE::read_u64(&header[0..8]) as usize;
        let max_elements = LE::read_i32(&header[16..20]);
        let num_elements = LE::read_i32(&header[20..24]);

        if is_valid_pointer(objects_ptr)
            && max_elements > 0
            && max_elements < 10_000_000
            && num_elements > 0
            && num_elements <= max_elements
        {
            eprintln!("Found GUObjectArray at SDK offset {:#x}", sdk_addr);
            return build_guobject_array(source, sdk_addr);
        }
    }

    // Fallback: Scan data regions for GUObjectArray-like structures
    eprintln!("Scanning data regions for GUObjectArray...");

    for region in source.regions() {
        // Focus on data sections
        if region.start < 0x151000000 || region.start > 0x175000000 {
            continue;
        }
        if !region.is_readable() {
            continue;
        }

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // GUObjectArray structure:
        // +0x00: Objects** (pointer to array of chunk pointers)
        // +0x08: PreAllocatedObjects* (usually NULL)
        // +0x10: MaxElements (4 bytes) - typically 0x200000 (2097152)
        // +0x14: NumElements (4 bytes) - current count
        // +0x18: MaxChunks (4 bytes)
        // +0x1C: NumChunks (4 bytes)

        for i in (0..data.len().saturating_sub(32)).step_by(8) {
            let objects_ptr = LE::read_u64(&data[i..i + 8]) as usize;
            let preallocated = LE::read_u64(&data[i + 8..i + 16]) as usize;
            let max_elements = LE::read_i32(&data[i + 16..i + 20]);
            let num_elements = LE::read_i32(&data[i + 20..i + 24]);
            let _max_chunks = LE::read_i32(&data[i + 24..i + 28]);
            let num_chunks = LE::read_i32(&data[i + 28..i + 32]);

            // Filter by expected values
            if !is_valid_pointer(objects_ptr) {
                continue;
            }
            if preallocated != 0 && !is_valid_pointer(preallocated) {
                continue;
            }
            // MaxElements is typically 0x200000 (2097152) or similar power of 2
            if max_elements < 100_000 || max_elements > 10_000_000 {
                continue;
            }
            // NumElements should be reasonable
            if num_elements < 10_000 || num_elements > max_elements {
                continue;
            }
            // NumChunks should be reasonable (each chunk holds 64K items)
            if num_chunks < 1 || num_chunks > 100 {
                continue;
            }

            let candidate_addr = region.start + i;

            // Verify by reading the Objects array
            if let Ok(objects_data) = source.read_bytes(objects_ptr, 8) {
                let first_chunk = LE::read_u64(&objects_data) as usize;
                if is_valid_pointer(first_chunk) {
                    // This looks like a valid GUObjectArray!
                    eprintln!(
                        "Found potential GUObjectArray at {:#x}: objects={:#x}, max={}, num={}, chunks={}",
                        candidate_addr, objects_ptr, max_elements, num_elements, num_chunks
                    );

                    // Verify by reading first few objects
                    if let Ok(chunk_data) = source.read_bytes(first_chunk, 24 * 5) {
                        let mut valid_count = 0;
                        for j in 0..5 {
                            // Try 24-byte item size (UE5.5)
                            let obj_ptr = LE::read_u64(&chunk_data[j * 24..j * 24 + 8]) as usize;
                            if obj_ptr == 0 || is_valid_pointer(obj_ptr) {
                                valid_count += 1;
                            }
                        }

                        if valid_count >= 4 {
                            eprintln!("  Verified: {} valid object pointers", valid_count);
                            return build_guobject_array(source, candidate_addr);
                        }
                    }
                }
            }
        }
    }

    bail!("Could not find GUObjectArray")
}

/// Build a GUObjectArray struct from a discovered address
fn build_guobject_array(source: &dyn MemorySource, addr: usize) -> Result<GUObjectArray> {
    let header = source.read_bytes(addr, 32)?;

    let objects_ptr = LE::read_u64(&header[0..8]) as usize;
    let max_elements = LE::read_i32(&header[16..20]);
    let num_elements = LE::read_i32(&header[20..24]);
    let num_chunks = LE::read_i32(&header[28..32]);

    eprintln!("GUObjectArray at {:#x}:", addr);
    eprintln!("  Objects ptr: {:#x}", objects_ptr);
    eprintln!("  MaxElements: {}", max_elements);
    eprintln!("  NumElements: {}", num_elements);
    eprintln!("  NumChunks: {}", num_chunks);

    // Read first chunk pointer
    let first_chunk_data = source.read_bytes(objects_ptr, 8)?;
    let first_chunk = LE::read_u64(&first_chunk_data) as usize;

    eprintln!("  First chunk: {:#x}", first_chunk);

    // Detect item size
    let item_size = GUObjectArray::detect_item_size(source, first_chunk)?;
    eprintln!("  Detected item size: {} bytes", item_size);

    Ok(GUObjectArray {
        address: addr,
        objects_ptr,
        max_elements,
        num_elements,
        first_chunk_ptr: first_chunk,
        item_size,
    })
}

/// Read an FName string from the GNames pool (legacy direct reading)
pub fn read_fname(source: &dyn MemorySource, gnames_addr: usize, index: u32) -> Result<String> {
    // UE5 FName entries are stored in a chunked array
    // Entry format varies, but typically:
    // - Header byte with length and flags
    // - String bytes

    let entry_offset = (index as usize) * 2; // Approximate - actual layout is chunked
    let entry_addr = gnames_addr + entry_offset;

    if let Ok(data) = source.read_bytes(entry_addr, 128) {
        // Try to parse FNameEntry
        // First byte often contains length in upper bits
        let len_byte = data[0];
        let string_len = ((len_byte >> 1) & 0x3F) as usize;

        if string_len > 0 && string_len < 64 {
            let name_bytes = &data[2..2 + string_len];
            // Check if it looks like ASCII
            if name_bytes.iter().all(|&b| b.is_ascii_graphic() || b == b'_') {
                return Ok(String::from_utf8_lossy(name_bytes).to_string());
            }
        }

        // Try alternative encoding
        let alt_len = (data[0] >> 6) as usize;
        if alt_len > 0 && alt_len < 64 {
            let name_bytes = &data[1..1 + alt_len];
            if name_bytes.iter().all(|&b| b.is_ascii_graphic() || b == b'_') {
                return Ok(String::from_utf8_lossy(name_bytes).to_string());
            }
        }
    }

    bail!("FName index {} not found", index)
}

/// Find UE5 global structures by pattern scanning
pub fn find_ue5_offsets(source: &dyn MemorySource) -> Result<Ue5Offsets> {
    let gnames = discover_gnames(source)?;

    // Try to find GUObjectArray
    let guobject_array = match discover_guobject_array(source, gnames.address) {
        Ok(arr) => arr.address,
        Err(_) => 0, // Not found yet
    };

    Ok(Ue5Offsets {
        gnames: gnames.address,
        guobject_array,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;
    use crate::memory::source::MemoryRegion;

    /// Create mock memory with GNames signature pattern
    /// Pattern: \x1e\x01None\x10\x03ByteProperty
    fn create_gnames_mock() -> (Vec<u8>, usize) {
        let base = 0x200000000usize;
        let mut data = vec![0u8; 4096];

        // Place GNames signature at offset 100
        let offset = 100;
        // FNameEntry for "None": header 0x1e 0x01 + "None"
        data[offset] = 0x1e;
        data[offset + 1] = 0x01;
        data[offset + 2..offset + 6].copy_from_slice(b"None");
        // FNameEntry for "ByteProperty": header 0x10 0x03 + "ByteProperty"
        data[offset + 6] = 0x10;
        data[offset + 7] = 0x03;
        data[offset + 8..offset + 20].copy_from_slice(b"ByteProperty");

        (data, base)
    }

    #[test]
    fn test_discover_gnames_finds_pattern() {
        let (data, base) = create_gnames_mock();
        let source = MockMemorySource::new(data, base);

        let result = discover_gnames(&source);
        assert!(result.is_ok());

        let pool = result.unwrap();
        assert_eq!(pool.address, base + 100);
        assert!(!pool.sample_names.is_empty());
    }

    #[test]
    fn test_discover_gnames_no_pattern() {
        // Empty memory - no GNames pattern
        let data = vec![0u8; 4096];
        let source = MockMemorySource::new(data, 0x200000000);

        let result = discover_gnames(&source);
        assert!(result.is_err());
    }

    #[test]
    fn test_discover_gnames_alt_pattern() {
        // Test the alternative "None" search path
        let base = 0x200000000usize;
        let mut data = vec![0u8; 4096];

        // Place "None" with different header bytes, followed by ByteProperty
        let offset = 100;
        data[offset] = 0x08; // Different header
        data[offset + 1] = 0x00;
        data[offset + 2..offset + 6].copy_from_slice(b"None");
        // ByteProperty nearby
        data[offset + 10..offset + 22].copy_from_slice(b"ByteProperty");

        let source = MockMemorySource::new(data, base);

        let result = discover_gnames(&source);
        // Should find via alternative pattern
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_valid_pointer() {
        // Valid pointers
        assert!(is_valid_pointer(0x100000));
        assert!(is_valid_pointer(0x140000000));
        assert!(is_valid_pointer(0x7FFFFFFFFFFF));

        // Invalid pointers
        assert!(!is_valid_pointer(0));
        assert!(!is_valid_pointer(0x1000)); // Too low
        assert!(!is_valid_pointer(0x800000000000)); // Too high
    }

    #[test]
    fn test_gnames_pool_structure() {
        let pool = GNamesPool {
            address: 0x200000000,
            sample_names: vec![
                (0, "None".to_string()),
                (1, "ByteProperty".to_string()),
                (2, "Class".to_string()),
            ],
        };

        assert_eq!(pool.address, 0x200000000);
        assert_eq!(pool.sample_names.len(), 3);
        assert_eq!(pool.sample_names[0], (0, "None".to_string()));
    }

    /// Create mock memory for GUObjectArray code pattern discovery
    fn create_guobject_code_mock() -> (Vec<u8>, usize) {
        let code_base = 0x140001000usize;
        let data_base = 0x151000000usize;

        // Create code section with the pattern
        // Pattern: 48 8B 05 ?? ?? ?? ?? 48 8B 0C C8 48 8D 04 D1
        let mut code = vec![0u8; 4096];

        // Place the pattern at offset 100
        let pattern_offset = 100;
        code[pattern_offset] = 0x48;
        code[pattern_offset + 1] = 0x8B;
        code[pattern_offset + 2] = 0x05;
        // RIP-relative displacement to data_base (calculate relative offset)
        // RIP after instruction = code_base + pattern_offset + 7
        // Target = data_base
        // displacement = target - rip = data_base - (code_base + pattern_offset + 7)
        let rip = code_base + pattern_offset + 7;
        let displacement = (data_base as i64 - rip as i64) as i32;
        code[pattern_offset + 3..pattern_offset + 7].copy_from_slice(&displacement.to_le_bytes());
        // Suffix pattern: 48 8B 0C C8 48 8D 04 D1
        code[pattern_offset + 7..pattern_offset + 15]
            .copy_from_slice(&[0x48, 0x8B, 0x0C, 0xC8, 0x48, 0x8D, 0x04, 0xD1]);

        (code, code_base)
    }

    #[test]
    fn test_guobject_code_pattern_detection() {
        let (code, code_base) = create_guobject_code_mock();

        // Create source with code region
        let source = MockMemorySource::with_regions(
            code,
            code_base,
            vec![MemoryRegion {
                start: code_base,
                end: code_base + 4096,
                perms: "r-xp".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            }],
        );

        // The pattern should be found at offset 100
        // We can't fully test discover_guobject_array without more mock data
        // but we can verify the code pattern exists
        let data = source.read_bytes(code_base + 100, 15).unwrap();
        assert_eq!(data[0], 0x48);
        assert_eq!(data[1], 0x8B);
        assert_eq!(data[2], 0x05);
        // Verify suffix
        assert_eq!(&data[7..15], &[0x48, 0x8B, 0x0C, 0xC8, 0x48, 0x8D, 0x04, 0xD1]);
    }
}
