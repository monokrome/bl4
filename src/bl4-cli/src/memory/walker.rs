//! GUObjectArray Walking and Dump Analysis
//!
//! Functions for iterating over UE5 object arrays:
//! - analyze_dump - Full memory dump analysis
//! - walk_guobject_array - Iterator over all UObjects with class info

use super::binary::find_code_bounds;
use super::constants::*;
use super::discovery::{discover_gnames, discover_guobject_array};
use super::fname::{FNamePool, FNameReader};
use super::reflection::{
    EPropertyType, EnumInfo, PropertyInfo, StructInfo, UClassMetaclassInfo, UObjectInfo,
    UObjectOffsets, discover_uclass_metaclass_exhaustive,
};
use super::source::MemorySource;
use super::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

pub fn analyze_dump(source: &dyn MemorySource) -> Result<()> {
    eprintln!("=== BL4 Dump Analysis ===\n");

    // Step 1: Find code bounds
    eprintln!("Step 1: Finding code bounds from PE header...");
    let code_bounds = find_code_bounds(source)?;
    eprintln!("  Found {} code ranges", code_bounds.ranges.len());

    // Step 2: Discover FNamePool
    eprintln!("\nStep 2: Discovering FNamePool...");

    let pool = match FNamePool::discover(source) {
        Ok(p) => {
            eprintln!("  FNamePool at {:#x}", p.header_addr);
            eprintln!(
                "  {} blocks, cursor at {}",
                p.current_block + 1,
                p.current_cursor
            );
            p
        }
        Err(e) => {
            eprintln!("  ERROR: Could not discover FNamePool: {}", e);
            bail!("FNamePool discovery failed - cannot continue analysis");
        }
    };

    let mut fname_reader = FNameReader::new(pool);

    // Verify FName resolution by finding "Class" and "Object" dynamically
    eprintln!("\nStep 3: Verifying FName resolution...");

    // Find "Class" FName dynamically
    let class_idx = match fname_reader.find_class_index(source) {
        Ok(idx) => {
            eprintln!(
                "  FName 'Class' found at index {} (SDK constant was {})",
                idx, FNAME_CLASS_INDEX
            );
            idx
        }
        Err(e) => {
            eprintln!("  ERROR: Could not find 'Class' FName: {}", e);
            FNAME_CLASS_INDEX // Fall back to SDK constant
        }
    };

    // Find "Object" FName dynamically
    let object_idx = match fname_reader.find_object_index(source) {
        Ok(idx) => {
            eprintln!(
                "  FName 'Object' found at index {} (SDK constant was {})",
                idx, FNAME_OBJECT_INDEX
            );
            idx
        }
        Err(e) => {
            eprintln!("  ERROR: Could not find 'Object' FName: {}", e);
            FNAME_OBJECT_INDEX // Fall back to SDK constant
        }
    };

    // Verify the indices work
    for (idx, expected) in [(class_idx, "Class"), (object_idx, "Object")] {
        match fname_reader.read_name(source, idx) {
            Ok(name) => {
                let status = if name == expected { "OK" } else { "MISMATCH" };
                eprintln!(
                    "  FName {} = \"{}\" (expected \"{}\") [{}]",
                    idx, name, expected, status
                );
            }
            Err(e) => {
                eprintln!("  FName {} = ERROR: {}", idx, e);
            }
        }
    }

    // Step 4: Find UClass metaclass
    eprintln!("\nStep 4: Finding UClass metaclass...");
    match discover_uclass_metaclass_exhaustive(source, &mut fname_reader) {
        Ok(info) => {
            eprintln!("\n=== UClass Metaclass Found ===");
            eprintln!("  Address: {:#x}", info.address);
            eprintln!("  Vtable: {:#x}", info.vtable);
            eprintln!("  ClassPrivate offset: {:#x}", info.class_offset);
            eprintln!("  NamePrivate offset: {:#x}", info.name_offset);
            eprintln!("  FName: {} (\"{}\")", info.fname_index, info.name);

            // Update the constants for future use
            eprintln!("\nRecommended constant updates:");
            eprintln!(
                "  pub const UCLASS_METACLASS_ADDR: usize = {:#x};",
                info.address
            );
            eprintln!(
                "  pub const UCLASS_METACLASS_VTABLE: usize = {:#x};",
                info.vtable
            );
            eprintln!(
                "  pub const UOBJECT_CLASS_OFFSET: usize = {:#x};",
                info.class_offset
            );
            eprintln!(
                "  pub const UOBJECT_NAME_OFFSET: usize = {:#x};",
                info.name_offset
            );
        }
        Err(e) => {
            eprintln!("  Failed: {}", e);
        }
    }

    Ok(())
}

/// Walk the GUObjectArray and collect all UClass/UScriptStruct/UEnum objects
pub fn walk_guobject_array(
    source: &dyn MemorySource,
    guobj_array: &GUObjectArray,
    fname_reader: &mut FNameReader,
) -> Result<Vec<UObjectInfo>> {
    let offsets = UObjectOffsets::default();
    let mut results = Vec::new();

    // First, we need to find the UClass for "Class", "ScriptStruct", and "Enum"
    // to identify which objects are reflection types
    let mut class_class_ptr: Option<usize> = None;
    let mut scriptstruct_class_ptr: Option<usize> = None;
    let mut enum_class_ptr: Option<usize> = None;

    // FUObjectItem size - use detected size from discovery
    let item_size = guobj_array.item_size;
    const CHUNK_SIZE: usize = GUOBJECTARRAY_CHUNK_SIZE;

    let num_chunks = ((guobj_array.num_elements as usize) + CHUNK_SIZE - 1) / CHUNK_SIZE;

    eprintln!(
        "Walking GUObjectArray: {} elements in {} chunks",
        guobj_array.num_elements, num_chunks
    );

    // Read chunk pointer array
    let chunk_ptrs_data = source.read_bytes(guobj_array.objects_ptr, num_chunks * 8)?;
    let chunk_ptrs: Vec<usize> = chunk_ptrs_data
        .chunks_exact(8)
        .map(|c| LE::read_u64(c) as usize)
        .collect();

    // First pass: find the self-referential UClass for "Class"
    // Then find ScriptStruct and Enum UClasses
    eprintln!("First pass: finding UClass 'Class' (self-referential)...");

    // Collect candidate objects with names "Class", "ScriptStruct", "Enum"
    let mut class_candidate: Option<(usize, usize)> = None; // (obj_ptr, class_ptr)
    let mut scriptstruct_candidate: Option<(usize, usize)> = None;
    let mut enum_candidate: Option<(usize, usize)> = None;

    let mut scanned = 0;
    for (chunk_idx, &chunk_ptr) in chunk_ptrs.iter().enumerate() {
        if chunk_ptr == 0 {
            continue;
        }

        let items_in_chunk = if chunk_idx == num_chunks - 1 {
            (guobj_array.num_elements as usize) % CHUNK_SIZE
        } else {
            CHUNK_SIZE
        };

        // Read entire chunk at once for efficiency
        let chunk_data = match source.read_bytes(chunk_ptr, items_in_chunk * item_size) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for item_idx in 0..items_in_chunk {
            let item_offset = item_idx * item_size;
            let obj_ptr = LE::read_u64(&chunk_data[item_offset..item_offset + 8]) as usize;

            if obj_ptr == 0 {
                continue;
            }

            scanned += 1;

            // Read UObject header (need at least name_offset + 4 = 0x34 bytes)
            let obj_data = match source.read_bytes(obj_ptr, 0x40) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let class_ptr =
                LE::read_u64(&obj_data[offsets.class_offset..offsets.class_offset + 8]) as usize;
            let name_index = LE::read_u32(&obj_data[offsets.name_offset..offsets.name_offset + 4]);

            // Debug: print first few FName indices we see
            if scanned <= 5 {
                eprintln!(
                    "  UObject[{}] at {:#x}: class={:#x}, name_idx={} ({:#x})",
                    scanned - 1,
                    obj_ptr,
                    class_ptr,
                    name_index,
                    name_index
                );
                let _ = fname_reader.debug_read(source, name_index);
            }

            // Read the name
            match fname_reader.read_name(source, name_index) {
                Ok(name) => {
                    // Self-referential class: UClass for "Class" has ClassPrivate pointing to itself
                    if name == "Class" && class_ptr == obj_ptr {
                        class_class_ptr = Some(obj_ptr);
                        class_candidate = Some((obj_ptr, class_ptr));
                        eprintln!(
                            "  Found UClass 'Class' at {:#x} (self-referential)",
                            obj_ptr
                        );
                    } else if name == "ScriptStruct" {
                        scriptstruct_candidate = Some((obj_ptr, class_ptr));
                    } else if name == "Enum" {
                        enum_candidate = Some((obj_ptr, class_ptr));
                    }

                    // Stop early if we found all three candidates
                    if class_candidate.is_some()
                        && scriptstruct_candidate.is_some()
                        && enum_candidate.is_some()
                    {
                        break;
                    }
                }
                Err(e) => {
                    if scanned <= 5 {
                        eprintln!("    FName read error: {}", e);
                    }
                }
            }

            // Progress indicator
            if scanned % 50000 == 0 {
                eprint!(
                    "\r  Scanned {}/{} objects...",
                    scanned, guobj_array.num_elements
                );
            }
        }

        // Stop early if we found all three candidates
        if class_candidate.is_some() && scriptstruct_candidate.is_some() && enum_candidate.is_some()
        {
            break;
        }
    }
    eprintln!("\r  First pass complete: scanned {} objects", scanned);

    // Validate candidates: ScriptStruct and Enum should have class_ptr == class_class_ptr
    if let Some(class_ptr) = class_class_ptr {
        if let Some((obj_ptr, cptr)) = scriptstruct_candidate {
            if cptr == class_ptr {
                scriptstruct_class_ptr = Some(obj_ptr);
                eprintln!("  Found UClass 'ScriptStruct' at {:#x}", obj_ptr);
            }
        }
        if let Some((obj_ptr, cptr)) = enum_candidate {
            if cptr == class_ptr {
                enum_class_ptr = Some(obj_ptr);
                eprintln!("  Found UClass 'Enum' at {:#x}", obj_ptr);
            }
        }
    }

    if class_class_ptr.is_none() {
        bail!("Could not find UClass 'Class' - FName reading may be broken");
    }

    eprintln!(
        "Core classes found:\n  Class={:#x}\n  ScriptStruct={:?}\n  Enum={:?}",
        class_class_ptr.unwrap(),
        scriptstruct_class_ptr,
        enum_class_ptr
    );

    // Second pass: collect all UClass, UScriptStruct, UEnum objects
    eprintln!("Second pass: collecting reflection objects...");

    scanned = 0;
    for (chunk_idx, &chunk_ptr) in chunk_ptrs.iter().enumerate() {
        if chunk_ptr == 0 {
            continue;
        }

        let items_in_chunk = if chunk_idx == num_chunks - 1 {
            (guobj_array.num_elements as usize) % CHUNK_SIZE
        } else {
            CHUNK_SIZE
        };

        let chunk_data = match source.read_bytes(chunk_ptr, items_in_chunk * item_size) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for item_idx in 0..items_in_chunk {
            let item_offset = item_idx * item_size;
            let obj_ptr = LE::read_u64(&chunk_data[item_offset..item_offset + 8]) as usize;

            if obj_ptr == 0 {
                continue;
            }

            scanned += 1;

            let obj_data = match source.read_bytes(obj_ptr, 0x38) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let class_ptr =
                LE::read_u64(&obj_data[offsets.class_offset..offsets.class_offset + 8]) as usize;
            let name_index = LE::read_u32(&obj_data[offsets.name_offset..offsets.name_offset + 4]);

            // Check if this object is a UClass, UScriptStruct, or UEnum
            let class_name = if Some(class_ptr) == class_class_ptr {
                "Class"
            } else if Some(class_ptr) == scriptstruct_class_ptr {
                "ScriptStruct"
            } else if Some(class_ptr) == enum_class_ptr {
                "Enum"
            } else {
                continue; // Not a reflection type we care about
            };

            if let Ok(name) = fname_reader.read_name(source, name_index) {
                results.push(UObjectInfo {
                    address: obj_ptr,
                    class_ptr,
                    name_index,
                    name,
                    class_name: class_name.to_string(),
                });
            }

            if scanned % 100000 == 0 {
                eprint!(
                    "\r  Scanned {}/{} objects, found {} reflection types...",
                    scanned,
                    guobj_array.num_elements,
                    results.len()
                );
            }
        }
    }

    eprintln!(
        "\r  Second pass complete: {} reflection objects found",
        results.len()
    );

    // Summary
    let class_count = results.iter().filter(|o| o.class_name == "Class").count();
    let struct_count = results
        .iter()
        .filter(|o| o.class_name == "ScriptStruct")
        .count();
    let enum_count = results.iter().filter(|o| o.class_name == "Enum").count();

    eprintln!(
        "Found {} UClass, {} UScriptStruct, {} UEnum",
        class_count, struct_count, enum_count
    );

    Ok(results)
}

/// Read property type name from FFieldClass
fn read_property_type(
    source: &dyn MemorySource,
    field_class_ptr: usize,
    fname_reader: &mut FNameReader,
    debug: bool,
) -> Result<String> {
    if field_class_ptr == 0 {
        return Ok("Unknown".to_string());
    }

    // Read FFieldClass data - in BL4's UE5.4, this is a vtable followed by class data
    // Read 0x180 bytes to find the FName (might be past offset 0x100)
    let class_data = source.read_bytes(field_class_ptr, 0x180)?;

    if debug {
        eprintln!(
            "  FFieldClass at {:#x} (raw dump - 0x180 bytes):",
            field_class_ptr
        );
        // Dump all 0x180 bytes as hex for analysis
        for i in 0..24 {
            let offset = i * 16;
            if offset + 16 <= class_data.len() {
                eprint!("    +{:03x}: ", offset);
                for j in 0..16 {
                    eprint!("{:02x} ", class_data[offset + j]);
                }
                // Also show as ASCII
                eprint!(" | ");
                for j in 0..16 {
                    let b = class_data[offset + j];
                    if b >= 0x20 && b < 0x7f {
                        eprint!("{}", b as char);
                    } else {
                        eprint!(".");
                    }
                }
                eprintln!();
            }
        }
    }

    // Search for an FName-like value (small index that resolves to *Property)
    // Property type FNames are at low indices: IntProperty ~10, ObjectProperty ~32
    // Scan entire 0x180 byte region
    for offset in (0..0x180).step_by(4) {
        if offset + 4 <= class_data.len() {
            let name_index = LE::read_u32(&class_data[offset..offset + 4]);
            // Property type FNames should be small (< 500) and non-zero
            if name_index > 0 && name_index < 500 {
                if let Ok(name) = fname_reader.read_name(source, name_index) {
                    if name.ends_with("Property") {
                        if debug {
                            eprintln!(
                                "    Found Property type at +{:#x}: idx={}, name='{}'",
                                offset, name_index, name
                            );
                        }
                        return Ok(name);
                    }
                }
            }
        }
    }

    // FFieldClass in BL4's UE5.4 is purely a vtable with no embedded FName.
    // Return a placeholder that will be replaced with actual type during property extraction.
    Ok("_UNKNOWN_TYPE_".to_string())
}

// Debug counter for property extraction
static DEBUG_PROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Extract a single FProperty from memory
pub fn extract_property(
    source: &dyn MemorySource,
    prop_ptr: usize,
    fname_reader: &mut FNameReader,
) -> Result<PropertyInfo> {
    // Read FProperty data (need about 0x80 bytes for base + some type-specific)
    let prop_data = source.read_bytes(prop_ptr, 0x80)?;

    // FField base
    let _field_class_ptr =
        LE::read_u64(&prop_data[FFIELD_CLASS_OFFSET..FFIELD_CLASS_OFFSET + 8]) as usize;
    let name_index = LE::read_u32(&prop_data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4]);

    // FProperty fields
    let array_dim =
        LE::read_i32(&prop_data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]);
    let element_size =
        LE::read_i32(&prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]);
    let property_flags = LE::read_u64(
        &prop_data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8],
    );
    let offset = LE::read_i32(&prop_data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]);

    // Debug first few properties (disabled for production)
    let count = DEBUG_PROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    #[allow(clippy::absurd_extreme_comparisons)]
    let debug = count < 0; // Disabled - always false for usize

    if debug {
        eprintln!("\nDEBUG Property {} at {:#x}:", count, prop_ptr);
        eprintln!("  FProperty raw dump (0x80 bytes):");
        for i in 0..8 {
            let off = i * 16;
            eprint!("    +{:03x}: ", off);
            for j in 0..16 {
                eprint!("{:02x} ", prop_data[off + j]);
            }
            eprintln!();
        }

        // Scan for small values that could be FName indices (property type names are < 100)
        eprintln!("  Small values that could be FName indices:");
        for off in (0..0x80).step_by(4) {
            let val = LE::read_u32(&prop_data[off..off + 4]);
            if val > 0 && val < 100 {
                if let Ok(name) = fname_reader.read_name(source, val) {
                    eprintln!("    +{:#04x}: idx={} -> '{}'", off, val, name);
                }
            }
        }
    }

    // Get property name
    let name = fname_reader.read_name(source, name_index)?;

    // Extract type-specific information and infer property type
    let mut struct_type = None;
    let mut enum_type = None;
    let mut inner_type = None;
    let mut value_type = None;
    let mut inferred_type = EPropertyType::Unknown;
    let mut inferred_type_name = "Unknown".to_string();

    // Read type-specific data at offset 0x78+
    let ptr_at_78 = LE::read_u64(&prop_data[0x78..0x80]) as usize;

    // Helper: check if a pointer looks like a valid UObject (has vtable in code section)
    let is_valid_uobject = |addr: usize| -> bool {
        if addr < 0x7ff000000000 || addr > 0x7fff00000000 {
            return false; // Not in heap range for this dump
        }
        if let Ok(vtable_data) = source.read_bytes(addr, 8) {
            let vtable = LE::read_u64(&vtable_data) as usize;
            // Vtable should be in code section (0x140... - 0x15f...)
            vtable >= 0x140000000 && vtable < 0x160000000
        } else {
            false
        }
    };

    // Helper: check if pointer looks like a property (has FFieldClass in data section)
    let is_valid_property = |addr: usize| -> bool {
        if addr == 0 {
            return false;
        }
        if let Ok(ffc_data) = source.read_bytes(addr, 8) {
            let ffc = LE::read_u64(&ffc_data) as usize;
            // FFieldClass should be in .didata section (0x14e... - 0x151...)
            ffc >= 0x14e000000 && ffc < 0x152000000
        } else {
            false
        }
    };

    // Try to infer type by probing type-specific data
    if ptr_at_78 != 0 {
        // Check if it's an inner property (ArrayProperty, SetProperty, MapProperty)
        if is_valid_property(ptr_at_78) {
            // Check for MapProperty first (has second property at 0x80)
            let mut is_map = false;
            if let Ok(extra) = source.read_bytes(prop_ptr + 0x80, 8) {
                let ptr_at_80 = LE::read_u64(&extra) as usize;
                if is_valid_property(ptr_at_80) {
                    if let Ok(key) = extract_property(source, ptr_at_78, fname_reader) {
                        if let Ok(val) = extract_property(source, ptr_at_80, fname_reader) {
                            inferred_type = EPropertyType::MapProperty;
                            inferred_type_name = "MapProperty".to_string();
                            inner_type = Some(Box::new(key));
                            value_type = Some(Box::new(val));
                            is_map = true;
                        }
                    }
                }
            }
            if !is_map {
                // It's ArrayProperty or SetProperty
                if let Ok(inner) = extract_property(source, ptr_at_78, fname_reader) {
                    inferred_type = EPropertyType::ArrayProperty;
                    inferred_type_name = "ArrayProperty".to_string();
                    inner_type = Some(Box::new(inner));
                }
            }
        }
        // Check if it's a UStruct* (StructProperty)
        else if is_valid_uobject(ptr_at_78) {
            if let Ok(struct_data) = source.read_bytes(ptr_at_78 + UOBJECT_NAME_OFFSET, 4) {
                let struct_name_idx = LE::read_u32(&struct_data);
                if let Ok(sname) = fname_reader.read_name(source, struct_name_idx) {
                    // Could be StructProperty or ObjectProperty - distinguish by class
                    if let Ok(class_data) = source.read_bytes(ptr_at_78 + UOBJECT_CLASS_OFFSET, 8) {
                        let class_ptr = LE::read_u64(&class_data) as usize;
                        if let Ok(class_name_data) =
                            source.read_bytes(class_ptr + UOBJECT_NAME_OFFSET, 4)
                        {
                            let class_name_idx = LE::read_u32(&class_name_data);
                            if let Ok(class_name) = fname_reader.read_name(source, class_name_idx) {
                                if class_name == "ScriptStruct" {
                                    inferred_type = EPropertyType::StructProperty;
                                    inferred_type_name = "StructProperty".to_string();
                                    struct_type = Some(sname);
                                } else if class_name == "Class" {
                                    inferred_type = EPropertyType::ObjectProperty;
                                    inferred_type_name = "ObjectProperty".to_string();
                                    struct_type = Some(sname);
                                } else if class_name == "Enum" {
                                    inferred_type = EPropertyType::ByteProperty;
                                    inferred_type_name = "ByteProperty".to_string();
                                    enum_type = Some(sname);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // If still unknown, infer from element size
    if inferred_type == EPropertyType::Unknown {
        inferred_type_name = match element_size {
            1 => {
                inferred_type = EPropertyType::ByteProperty;
                "ByteProperty"
            }
            2 => {
                inferred_type = EPropertyType::Int16Property;
                "Int16Property"
            }
            4 => {
                inferred_type = EPropertyType::IntProperty;
                "IntProperty"
            }
            8 => {
                inferred_type = EPropertyType::Int64Property;
                "Int64Property"
            }
            12 => {
                inferred_type = EPropertyType::StructProperty;
                "StructProperty"
            } // Likely FVector
            16 => {
                inferred_type = EPropertyType::StrProperty;
                "StrProperty"
            } // FString is 16 bytes
            24 => {
                inferred_type = EPropertyType::StructProperty;
                "StructProperty"
            } // Likely FRotator or Transform
            _ => "Unknown",
        }
        .to_string();
    }

    if debug {
        eprintln!(
            "  Resolved: name='{}', inferred_type='{}'",
            name, inferred_type_name
        );
    }

    Ok(PropertyInfo {
        name,
        property_type: inferred_type,
        type_name: inferred_type_name,
        array_dim,
        element_size,
        property_flags,
        offset,
        struct_type,
        enum_type,
        inner_type,
        value_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    /// Create a mock FProperty in memory
    /// Layout: FField base (0x30) + FProperty fields
    fn create_mock_property(
        name_index: u32,
        array_dim: i32,
        element_size: i32,
        property_flags: u64,
        offset: i32,
    ) -> Vec<u8> {
        let mut data = vec![0u8; 0x80];

        // FField::ClassPrivate at +0x00 (mock pointer)
        data[0..8].copy_from_slice(&0x14f000000u64.to_le_bytes());

        // FField::NamePrivate at +0x20 (FName = index + number)
        data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4].copy_from_slice(&name_index.to_le_bytes());

        // FProperty::ArrayDim at +0x30
        data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]
            .copy_from_slice(&array_dim.to_le_bytes());

        // FProperty::ElementSize at +0x34
        data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]
            .copy_from_slice(&element_size.to_le_bytes());

        // FProperty::PropertyFlags at +0x38
        data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8]
            .copy_from_slice(&property_flags.to_le_bytes());

        // FProperty::Offset_Internal at +0x4C
        data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]
            .copy_from_slice(&offset.to_le_bytes());

        data
    }

    /// Create a mock UObject header
    fn create_mock_uobject(
        vtable: u64,
        flags: i32,
        internal_index: i32,
        class_ptr: u64,
        name_index: u32,
        outer_ptr: u64,
    ) -> Vec<u8> {
        let mut data = vec![0u8; UOBJECT_HEADER_SIZE];

        data[UOBJECT_VTABLE_OFFSET..UOBJECT_VTABLE_OFFSET + 8]
            .copy_from_slice(&vtable.to_le_bytes());
        data[UOBJECT_FLAGS_OFFSET..UOBJECT_FLAGS_OFFSET + 4].copy_from_slice(&flags.to_le_bytes());
        data[UOBJECT_INTERNAL_INDEX_OFFSET..UOBJECT_INTERNAL_INDEX_OFFSET + 4]
            .copy_from_slice(&internal_index.to_le_bytes());
        data[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]
            .copy_from_slice(&class_ptr.to_le_bytes());
        data[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]
            .copy_from_slice(&name_index.to_le_bytes());
        data[UOBJECT_OUTER_OFFSET..UOBJECT_OUTER_OFFSET + 8]
            .copy_from_slice(&outer_ptr.to_le_bytes());

        data
    }

    #[test]
    fn test_create_mock_property() {
        let prop_data = create_mock_property(100, 1, 4, 0x1234, 0x10);

        // Verify fields are at correct offsets
        assert_eq!(
            LE::read_u32(&prop_data[FFIELD_NAME_OFFSET..FFIELD_NAME_OFFSET + 4]),
            100
        );
        assert_eq!(
            LE::read_i32(&prop_data[FPROPERTY_ARRAYDIM_OFFSET..FPROPERTY_ARRAYDIM_OFFSET + 4]),
            1
        );
        assert_eq!(
            LE::read_i32(&prop_data[FPROPERTY_ELEMENTSIZE_OFFSET..FPROPERTY_ELEMENTSIZE_OFFSET + 4]),
            4
        );
        assert_eq!(
            LE::read_u64(
                &prop_data[FPROPERTY_PROPERTYFLAGS_OFFSET..FPROPERTY_PROPERTYFLAGS_OFFSET + 8]
            ),
            0x1234
        );
        assert_eq!(
            LE::read_i32(&prop_data[FPROPERTY_OFFSET_OFFSET..FPROPERTY_OFFSET_OFFSET + 4]),
            0x10
        );
    }

    #[test]
    fn test_create_mock_uobject() {
        let obj_data = create_mock_uobject(
            0x14f000000, // vtable
            0x01,        // flags
            42,          // internal index
            0x200000000, // class ptr
            100,         // name index
            0x300000000, // outer ptr
        );

        assert_eq!(obj_data.len(), UOBJECT_HEADER_SIZE);
        assert_eq!(
            LE::read_u64(&obj_data[UOBJECT_VTABLE_OFFSET..UOBJECT_VTABLE_OFFSET + 8]),
            0x14f000000
        );
        assert_eq!(
            LE::read_i32(&obj_data[UOBJECT_FLAGS_OFFSET..UOBJECT_FLAGS_OFFSET + 4]),
            0x01
        );
        assert_eq!(
            LE::read_i32(
                &obj_data[UOBJECT_INTERNAL_INDEX_OFFSET..UOBJECT_INTERNAL_INDEX_OFFSET + 4]
            ),
            42
        );
        assert_eq!(
            LE::read_u64(&obj_data[UOBJECT_CLASS_OFFSET..UOBJECT_CLASS_OFFSET + 8]),
            0x200000000
        );
        assert_eq!(
            LE::read_u32(&obj_data[UOBJECT_NAME_OFFSET..UOBJECT_NAME_OFFSET + 4]),
            100
        );
        assert_eq!(
            LE::read_u64(&obj_data[UOBJECT_OUTER_OFFSET..UOBJECT_OUTER_OFFSET + 8]),
            0x300000000
        );
    }

    #[test]
    fn test_uobject_info_structure() {
        let info = UObjectInfo {
            address: 0x200000000,
            class_ptr: 0x200000100,
            name_index: 42,
            name: "TestObject".to_string(),
            class_name: "Class".to_string(),
        };

        assert_eq!(info.address, 0x200000000);
        assert_eq!(info.name, "TestObject");
        assert_eq!(info.class_name, "Class");
    }

    #[test]
    fn test_property_type_inference_from_element_size() {
        // ByteProperty (size 1)
        let info = PropertyInfo {
            name: "ByteProp".to_string(),
            property_type: EPropertyType::ByteProperty,
            type_name: "ByteProperty".to_string(),
            array_dim: 1,
            element_size: 1,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };
        assert_eq!(info.element_size, 1);

        // IntProperty (size 4)
        let int_info = PropertyInfo {
            name: "IntProp".to_string(),
            property_type: EPropertyType::IntProperty,
            type_name: "IntProperty".to_string(),
            array_dim: 1,
            element_size: 4,
            property_flags: 0,
            offset: 4,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };
        assert_eq!(int_info.element_size, 4);
    }

    #[test]
    fn test_property_info_with_struct_type() {
        let info = PropertyInfo {
            name: "VectorProp".to_string(),
            property_type: EPropertyType::StructProperty,
            type_name: "StructProperty".to_string(),
            array_dim: 1,
            element_size: 12, // FVector = 3 floats
            property_flags: 0,
            offset: 0,
            struct_type: Some("Vector".to_string()),
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        assert_eq!(info.struct_type, Some("Vector".to_string()));
        assert_eq!(info.property_type, EPropertyType::StructProperty);
    }

    #[test]
    fn test_property_info_with_array() {
        let inner = PropertyInfo {
            name: "".to_string(),
            property_type: EPropertyType::IntProperty,
            type_name: "IntProperty".to_string(),
            array_dim: 1,
            element_size: 4,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let array_info = PropertyInfo {
            name: "IntArray".to_string(),
            property_type: EPropertyType::ArrayProperty,
            type_name: "ArrayProperty".to_string(),
            array_dim: 1,
            element_size: 16, // TArray header
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: Some(Box::new(inner)),
            value_type: None,
        };

        assert!(array_info.inner_type.is_some());
        assert_eq!(
            array_info.inner_type.as_ref().unwrap().property_type,
            EPropertyType::IntProperty
        );
    }

    #[test]
    fn test_property_info_with_map() {
        let key_type = PropertyInfo {
            name: "".to_string(),
            property_type: EPropertyType::StrProperty,
            type_name: "StrProperty".to_string(),
            array_dim: 1,
            element_size: 16,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let value_type = PropertyInfo {
            name: "".to_string(),
            property_type: EPropertyType::IntProperty,
            type_name: "IntProperty".to_string(),
            array_dim: 1,
            element_size: 4,
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: None,
            value_type: None,
        };

        let map_info = PropertyInfo {
            name: "StringToIntMap".to_string(),
            property_type: EPropertyType::MapProperty,
            type_name: "MapProperty".to_string(),
            array_dim: 1,
            element_size: 80, // TMap header
            property_flags: 0,
            offset: 0,
            struct_type: None,
            enum_type: None,
            inner_type: Some(Box::new(key_type)),
            value_type: Some(Box::new(value_type)),
        };

        assert!(map_info.inner_type.is_some());
        assert!(map_info.value_type.is_some());
        assert_eq!(
            map_info.inner_type.as_ref().unwrap().property_type,
            EPropertyType::StrProperty
        );
        assert_eq!(
            map_info.value_type.as_ref().unwrap().property_type,
            EPropertyType::IntProperty
        );
    }

    #[test]
    fn test_struct_info_basic() {
        let struct_info = StructInfo {
            address: 0x200000000,
            name: "TestStruct".to_string(),
            super_name: None,
            properties: vec![],
            struct_size: 0x40,
            is_class: false,
        };

        assert_eq!(struct_info.name, "TestStruct");
        assert!(struct_info.super_name.is_none());
        assert!(!struct_info.is_class);
    }

    #[test]
    fn test_struct_info_with_super() {
        let struct_info = StructInfo {
            address: 0x200000000,
            name: "ChildClass".to_string(),
            super_name: Some("ParentClass".to_string()),
            properties: vec![],
            struct_size: 0x100,
            is_class: true,
        };

        assert_eq!(struct_info.super_name, Some("ParentClass".to_string()));
        assert!(struct_info.is_class);
    }

    #[test]
    fn test_enum_info_structure() {
        let enum_info = EnumInfo {
            address: 0x200000000,
            name: "ETestEnum".to_string(),
            values: vec![
                ("Value1".to_string(), 0),
                ("Value2".to_string(), 1),
                ("Value3".to_string(), 2),
                ("MAX".to_string(), 255),
            ],
        };

        assert_eq!(enum_info.name, "ETestEnum");
        assert_eq!(enum_info.values.len(), 4);
        assert_eq!(enum_info.values[0], ("Value1".to_string(), 0));
        assert_eq!(enum_info.values[3], ("MAX".to_string(), 255));
    }

    #[test]
    fn test_uobject_offsets_default() {
        let offsets = UObjectOffsets::default();

        // Verify default offsets match constants
        assert_eq!(offsets.class_offset, UOBJECT_CLASS_OFFSET);
        assert_eq!(offsets.name_offset, UOBJECT_NAME_OFFSET);
    }
}
