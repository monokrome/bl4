//! GUObjectArray discovery
//!
//! Discovers GUObjectArray by scanning memory for the characteristic structure.

use super::super::constants::*;
use super::super::source::MemorySource;
use super::super::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// Check if a pointer value looks like a valid heap/data pointer
pub fn is_valid_pointer(ptr: usize) -> bool {
    ptr >= MIN_VALID_POINTER && ptr < MAX_VALID_POINTER
}

/// Validate a candidate GUObjectArray structure
fn validate_guobject_array(source: &dyn MemorySource, addr: usize) -> Option<(usize, i32, i32)> {
    let header = source.read_bytes(addr, 32).ok()?;

    let objects_ptr = LE::read_u64(&header[0..8]) as usize;
    let preallocated = LE::read_u64(&header[8..16]) as usize;
    let max_elements = LE::read_i32(&header[16..20]);
    let num_elements = LE::read_i32(&header[20..24]);
    let num_chunks = LE::read_i32(&header[28..32]);

    // Validate structure fields
    if !is_valid_pointer(objects_ptr) {
        return None;
    }
    if preallocated != 0 && !is_valid_pointer(preallocated) {
        return None;
    }
    // MaxElements is typically 0x200000 (2097152) or similar
    if max_elements < 100_000 || max_elements > 10_000_000 {
        return None;
    }
    if num_elements < 10_000 || num_elements > max_elements {
        return None;
    }
    // Each chunk holds 64K items
    if num_chunks < 1 || num_chunks > 100 {
        return None;
    }

    // Verify the Objects pointer points to valid chunk pointers
    let objects_data = source.read_bytes(objects_ptr, 8).ok()?;
    let first_chunk = LE::read_u64(&objects_data) as usize;
    if !is_valid_pointer(first_chunk) {
        return None;
    }

    // Verify first chunk contains valid object pointers that look like UObjects
    let chunk_data = source.read_bytes(first_chunk, 24 * 10).ok()?;
    let mut valid_count = 0;
    let mut uobject_count = 0;

    for j in 0..10 {
        let obj_ptr = LE::read_u64(&chunk_data[j * 24..j * 24 + 8]) as usize;
        if obj_ptr == 0 {
            valid_count += 1;
            continue;
        }
        if !is_valid_pointer(obj_ptr) {
            continue;
        }
        valid_count += 1;

        // Read potential UObject and verify vtable is in executable range
        if let Ok(obj_data) = source.read_bytes(obj_ptr, 0x20) {
            let vtable = LE::read_u64(&obj_data[0..8]) as usize;
            // vtable should be in executable/data section (0x140000000 - 0x160000000)
            if vtable >= 0x140000000 && vtable < 0x160000000 {
                uobject_count += 1;
            }
        }
    }

    // Need majority valid pointers AND at least some that look like real UObjects
    if valid_count < 7 || uobject_count < 3 {
        return None;
    }

    Some((objects_ptr, max_elements, num_elements))
}

/// Discover GUObjectArray by scanning all memory regions
///
/// Searches for the characteristic GUObjectArray structure:
/// - Objects**: pointer to array of chunk pointers
/// - MaxElements: ~2M (power of 2)
/// - NumElements: current count (10K-2M range)
/// - NumChunks: 1-100 range
pub fn discover_guobject_array(
    source: &dyn MemorySource,
    _gnames_addr: usize,
) -> Result<GUObjectArray> {
    eprintln!("Scanning memory for GUObjectArray structure...");

    // First pass: try code pattern scanning (most reliable when it works)
    // Pattern: mov rax, [rip+disp32]; mov rcx, [rax+rcx*8]; lea rax, [rcx+rdx*8]
    let pattern_suffix: &[u8] = &[0x48, 0x8B, 0x0C, 0xC8, 0x48, 0x8D, 0x04, 0xD1];

    for region in source.regions() {
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
            if &data[i + 7..i + 15] != pattern_suffix {
                continue;
            }

            // Extract RIP-relative displacement
            let disp = LE::read_i32(&data[i + 3..i + 7]) as i64;
            let code_addr = region.start + i;
            let rip = code_addr + 7;
            let guobj_addr = (rip as i64 + disp) as usize;

            if let Some((objects_ptr, max_elements, num_elements)) =
                validate_guobject_array(source, guobj_addr)
            {
                eprintln!(
                    "Found GUObjectArray at {:#x} via code pattern at {:#x}",
                    guobj_addr, code_addr
                );
                eprintln!(
                    "  Objects: {:#x}, Max: {}, Num: {}",
                    objects_ptr, max_elements, num_elements
                );
                return build_guobject_array(source, guobj_addr);
            }
        }
    }

    // Second pass: scan all regions for the structure pattern
    eprintln!("Code pattern not found, scanning data regions...");

    for region in source.regions() {
        if !region.is_readable() {
            continue;
        }

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for i in (0..data.len().saturating_sub(32)).step_by(8) {
            let candidate_addr = region.start + i;

            if let Some((objects_ptr, max_elements, num_elements)) =
                validate_guobject_array(source, candidate_addr)
            {
                eprintln!(
                    "Found GUObjectArray at {:#x}: objects={:#x}, max={}, num={}",
                    candidate_addr, objects_ptr, max_elements, num_elements
                );
                return build_guobject_array(source, candidate_addr);
            }
        }
    }

    bail!("Could not find GUObjectArray in memory dump")
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
