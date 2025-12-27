//! GUObjectArray discovery
//!
//! Discovers GUObjectArray by scanning code for the access pattern
//! and validating the structure.

use super::super::constants::*;
use super::super::source::MemorySource;
use super::super::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// Check if a pointer value looks like a valid heap/data pointer for this dump
/// Windows heap is typically in the range 0x00010000-0x7FFFFFFFFFFF
pub fn is_valid_pointer(ptr: usize) -> bool {
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
