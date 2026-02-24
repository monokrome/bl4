//! GUObjectArray discovery
//!
//! Discovers GUObjectArray using SDK constants first (fast path),
//! falling back to code pattern scanning and data region scanning.

use super::super::constants::*;
use super::super::source::MemorySource;
use super::super::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// Check if a pointer value looks like a valid heap/data pointer for this dump
pub fn is_valid_pointer(ptr: usize) -> bool {
    (MIN_VALID_POINTER..MAX_VALID_POINTER).contains(&ptr)
}

/// Discover GUObjectArray, trying SDK offset first before pattern scanning.
#[allow(clippy::cognitive_complexity)]
pub fn discover_guobject_array(
    source: &dyn MemorySource,
    _gnames_addr: usize,
) -> Result<GUObjectArray> {
    // Fast path: try the known SDK offset first
    let sdk_addr = PE_IMAGE_BASE + GOBJECTS_OFFSET;
    eprintln!("Trying SDK offset for GUObjectArray at {:#x}...", sdk_addr);

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

    // Slow path 1: code pattern scanning
    eprintln!("SDK offset failed, searching via code pattern...");

    let pattern_suffix: &[u8] = &[0x48, 0x8B, 0x0C, 0xC8, 0x48, 0x8D, 0x04, 0xD1];
    let mut found_candidates: Vec<(usize, usize)> = Vec::new();

    for region in source.regions() {
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

        for i in 0..data.len().saturating_sub(15) {
            if data[i] != 0x48 || data[i + 1] != 0x8B || data[i + 2] != 0x05 {
                continue;
            }

            if &data[i + 7..i + 15] != pattern_suffix {
                continue;
            }

            let disp = LE::read_i32(&data[i + 3..i + 7]) as i64;
            let code_addr = region.start + i;
            let rip = code_addr + 7;
            let guobj_addr = (rip as i64 + disp) as usize;

            if !is_valid_pointer(guobj_addr) {
                continue;
            }

            if let Ok(header) = source.read_bytes(guobj_addr, 32) {
                let objects_ptr = LE::read_u64(&header[0..8]) as usize;
                let max_elements = LE::read_i32(&header[16..20]);
                let num_elements = LE::read_i32(&header[20..24]);

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
                    found_candidates.push((code_addr, guobj_addr));
                }
            }
        }
    }

    if !found_candidates.is_empty() {
        let (_, guobj_addr) = found_candidates[0];
        return build_guobject_array(source, guobj_addr);
    }

    // Slow path 2: scan data regions for GUObjectArray-like structures
    eprintln!("Scanning data regions for GUObjectArray...");

    for region in source.regions() {
        // Search all data regions above 0x14c000000 (covers PE data sections and heap)
        if region.start < 0x14c000000 || region.start > 0x200000000 {
            continue;
        }
        if !region.is_readable() {
            continue;
        }

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for i in (0..data.len().saturating_sub(32)).step_by(8) {
            if !validate_guobject_candidate(source, &data, i, region.start) {
                continue;
            }

            let candidate_addr = region.start + i;
            let objects_ptr = LE::read_u64(&data[i..i + 8]) as usize;
            let max_elements = LE::read_i32(&data[i + 16..i + 20]);
            let num_elements = LE::read_i32(&data[i + 20..i + 24]);
            let num_chunks = LE::read_i32(&data[i + 28..i + 32]);

            eprintln!(
                "Found GUObjectArray at {:#x}: objects={:#x}, max={}, num={}, chunks={}",
                candidate_addr, objects_ptr, max_elements, num_elements, num_chunks
            );
            return build_guobject_array(source, candidate_addr);
        }
    }

    bail!("Could not find GUObjectArray")
}

/// Validate a candidate GUObjectArray at data[offset] within a memory region.
///
/// Checks: structural fields, chunk count consistency, multiple chunk pointer
/// validity, and deep UObject validation on items within the first chunk.
#[allow(clippy::cognitive_complexity)]
fn validate_guobject_candidate(
    source: &dyn MemorySource,
    data: &[u8],
    offset: usize,
    region_start: usize,
) -> bool {
    let objects_ptr = LE::read_u64(&data[offset..offset + 8]) as usize;
    let preallocated = LE::read_u64(&data[offset + 8..offset + 16]) as usize;
    let max_elements = LE::read_i32(&data[offset + 16..offset + 20]);
    let num_elements = LE::read_i32(&data[offset + 20..offset + 24]);
    let num_chunks = LE::read_i32(&data[offset + 28..offset + 32]);

    // Basic structural checks
    if !is_valid_pointer(objects_ptr) {
        return false;
    }
    if preallocated != 0 && !is_valid_pointer(preallocated) {
        return false;
    }
    if !(100_000..=10_000_000).contains(&max_elements) {
        return false;
    }
    if num_elements < 10_000 || num_elements > max_elements {
        return false;
    }
    if num_chunks < 1 {
        return false;
    }

    // Chunk count consistency: num_chunks must cover all elements
    let required_chunks = (num_elements as usize).div_ceil(GUOBJECTARRAY_CHUNK_SIZE);
    if (num_chunks as usize) < required_chunks {
        return false;
    }
    // Don't allow wildly excessive chunk counts either
    if (num_chunks as usize) > required_chunks + 2 {
        return false;
    }

    // Read and validate multiple chunk pointers (not just the first)
    let chunks_to_check = (num_chunks as usize).min(8);
    let chunk_ptrs_data = match source.read_bytes(objects_ptr, chunks_to_check * 8) {
        Ok(d) => d,
        Err(_) => return false,
    };

    for k in 0..chunks_to_check {
        let chunk_ptr = LE::read_u64(&chunk_ptrs_data[k * 8..(k + 1) * 8]) as usize;
        if !is_valid_pointer(chunk_ptr) {
            return false;
        }
    }

    // Deep validation: read items from first chunk and verify they look like UObjects.
    // Try multiple (stride, object_offset) combinations since the build may vary.
    let first_chunk = LE::read_u64(&chunk_ptrs_data[0..8]) as usize;
    let items_to_check = 5usize;
    // Read enough for the largest stride (24) * items + max offset (8) + pointer (8)
    let read_size = items_to_check * 24 + 8 + 8;
    let chunk_data = match source.read_bytes(first_chunk, read_size) {
        Ok(d) => d,
        Err(_) => return false,
    };

    // Try: (stride, object_ptr_offset_within_item)
    // Standard UE5: 24-byte items, Object* at +8
    // Compact UE5: 16-byte items, Object* at +8
    // Legacy/custom: 24-byte items, Object* at +0
    // Minimal: 16-byte items, Object* at +0
    let layouts: &[(usize, usize)] = &[(24, 8), (16, 8), (24, 0), (16, 0)];
    let mut best_valid = 0;

    for &(stride, obj_off) in layouts {
        let mut valid = 0;
        for j in 0..items_to_check {
            let off = j * stride + obj_off;
            if off + 8 > chunk_data.len() {
                break;
            }
            let obj_ptr = LE::read_u64(&chunk_data[off..off + 8]) as usize;
            if obj_ptr == 0 {
                continue;
            }
            if !is_valid_pointer(obj_ptr) {
                continue;
            }

            // Read UObject header and verify vtable + ClassPrivate
            if let Ok(uobj_data) = source.read_bytes(obj_ptr, UOBJECT_HEADER_SIZE) {
                let vtable = LE::read_u64(
                    &uobj_data[UOBJECT_VTABLE_OFFSET..UOBJECT_VTABLE_OFFSET + 8],
                ) as usize;
                let class_ptr = LE::read_u64(
                    &uobj_data[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8],
                ) as usize;

                if (MIN_VTABLE_ADDR..=MAX_VTABLE_ADDR).contains(&vtable)
                    && is_valid_pointer(class_ptr)
                {
                    valid += 1;
                }
            }
        }
        if valid > best_valid {
            best_valid = valid;
        }
    }

    if best_valid < 3 {
        let candidate_addr = region_start + offset;
        eprintln!(
            "  Rejected candidate at {:#x}: only {}/5 valid UObjects (need 3+)",
            candidate_addr, best_valid
        );
        return false;
    }

    true
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

    // Detect item layout (stride + object offset)
    let (item_size, object_offset) = GUObjectArray::detect_item_layout(source, first_chunk)?;

    Ok(GUObjectArray {
        address: addr,
        objects_ptr,
        max_elements,
        num_elements,
        first_chunk_ptr: first_chunk,
        item_size,
        object_offset,
    })
}
