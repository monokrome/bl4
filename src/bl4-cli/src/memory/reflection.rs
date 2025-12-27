//! UE5 Reflection Data Structures
//!
//! Types and functions for UE5 reflection system:
//! - UObject metadata (UObjectInfo)
//! - Property types (EPropertyType)
//! - Struct/Property/Enum info structures
//! - UClass discovery and metaclass analysis

use super::binary::find_code_bounds;
use super::constants::*;
use super::fname::FNameReader;
use super::source::MemorySource;
use super::ue5::GUObjectArray;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

pub struct UObjectInfo {
    pub address: usize,
    pub class_ptr: usize,
    pub name_index: u32,
    pub name: String,
    pub class_name: String,
}

/// Property type enumeration for usmap
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EPropertyType {
    ByteProperty,
    BoolProperty,
    IntProperty,
    FloatProperty,
    ObjectProperty,
    NameProperty,
    DelegateProperty,
    DoubleProperty,
    ArrayProperty,
    StructProperty,
    StrProperty,
    TextProperty,
    InterfaceProperty,
    MulticastDelegateProperty,
    WeakObjectProperty,
    LazyObjectProperty,
    AssetObjectProperty,
    SoftObjectProperty,
    UInt64Property,
    UInt32Property,
    UInt16Property,
    Int64Property,
    Int16Property,
    Int8Property,
    MapProperty,
    SetProperty,
    EnumProperty,
    FieldPathProperty,
    OptionalProperty,
    Unknown,
}

impl EPropertyType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "ByteProperty" => Self::ByteProperty,
            "BoolProperty" => Self::BoolProperty,
            "IntProperty" => Self::IntProperty,
            "FloatProperty" => Self::FloatProperty,
            "ObjectProperty" => Self::ObjectProperty,
            "NameProperty" => Self::NameProperty,
            "DelegateProperty" => Self::DelegateProperty,
            "DoubleProperty" => Self::DoubleProperty,
            "ArrayProperty" => Self::ArrayProperty,
            "StructProperty" => Self::StructProperty,
            "StrProperty" => Self::StrProperty,
            "TextProperty" => Self::TextProperty,
            "InterfaceProperty" => Self::InterfaceProperty,
            "MulticastDelegateProperty"
            | "MulticastInlineDelegateProperty"
            | "MulticastSparseDelegateProperty" => Self::MulticastDelegateProperty,
            "WeakObjectProperty" => Self::WeakObjectProperty,
            "LazyObjectProperty" => Self::LazyObjectProperty,
            "AssetObjectProperty" => Self::AssetObjectProperty,
            "SoftObjectProperty" => Self::SoftObjectProperty,
            "UInt64Property" => Self::UInt64Property,
            "UInt32Property" => Self::UInt32Property,
            "UInt16Property" => Self::UInt16Property,
            "Int64Property" => Self::Int64Property,
            "Int16Property" => Self::Int16Property,
            "Int8Property" => Self::Int8Property,
            "MapProperty" => Self::MapProperty,
            "SetProperty" => Self::SetProperty,
            "EnumProperty" => Self::EnumProperty,
            "FieldPathProperty" => Self::FieldPathProperty,
            "OptionalProperty" => Self::OptionalProperty,
            "ClassProperty" => Self::ObjectProperty, // ClassProperty is a subtype of ObjectProperty
            "SoftClassProperty" => Self::SoftObjectProperty,
            _ => Self::Unknown,
        }
    }

    /// Get the usmap type ID
    pub fn to_usmap_id(&self) -> u8 {
        match self {
            Self::ByteProperty => 0,
            Self::BoolProperty => 1,
            Self::IntProperty => 2,
            Self::FloatProperty => 3,
            Self::ObjectProperty => 4,
            Self::NameProperty => 5,
            Self::DelegateProperty => 6,
            Self::DoubleProperty => 7,
            Self::ArrayProperty => 8,
            Self::StructProperty => 9,
            Self::StrProperty => 10,
            Self::TextProperty => 11,
            Self::InterfaceProperty => 12,
            Self::MulticastDelegateProperty => 13,
            Self::WeakObjectProperty => 14,
            Self::LazyObjectProperty => 15,
            Self::AssetObjectProperty => 16,
            Self::SoftObjectProperty => 17,
            Self::UInt64Property => 18,
            Self::UInt32Property => 19,
            Self::UInt16Property => 20,
            Self::Int64Property => 21,
            Self::Int16Property => 22,
            Self::Int8Property => 23,
            Self::MapProperty => 24,
            Self::SetProperty => 25,
            Self::EnumProperty => 26,
            Self::FieldPathProperty => 27,
            Self::OptionalProperty => 28,
            Self::Unknown => 255,
        }
    }
}

/// Property information extracted from FProperty
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// Property name
    pub name: String,
    /// Property type (e.g., "IntProperty", "StructProperty")
    pub property_type: EPropertyType,
    /// Property type name string
    pub type_name: String,
    /// Array dimension (1 for regular, >1 for fixed arrays)
    pub array_dim: i32,
    /// Element size in bytes
    pub element_size: i32,
    /// Property flags (EPropertyFlags)
    pub property_flags: u64,
    /// Offset within struct
    pub offset: i32,
    /// For StructProperty: the struct type name
    pub struct_type: Option<String>,
    /// For EnumProperty: the enum type name
    pub enum_type: Option<String>,
    /// For ArrayProperty/SetProperty/MapProperty: inner property type
    pub inner_type: Option<Box<PropertyInfo>>,
    /// For MapProperty: value property type
    pub value_type: Option<Box<PropertyInfo>>,
}

/// UStruct/UClass with extracted properties
#[derive(Debug, Clone)]
pub struct StructInfo {
    /// Address of the UStruct in memory
    pub address: usize,
    /// Name of the struct/class
    pub name: String,
    /// Super class/struct name (if any)
    pub super_name: Option<String>,
    /// Properties in this struct
    pub properties: Vec<PropertyInfo>,
    /// Size of the struct in bytes
    pub struct_size: i32,
    /// Whether this is a UClass (vs UScriptStruct)
    pub is_class: bool,
}

/// Enum information
#[derive(Debug, Clone)]
pub struct EnumInfo {
    /// Address of the UEnum in memory
    pub address: usize,
    /// Name of the enum
    pub name: String,
    /// Enum values (name, value)
    pub values: Vec<(String, i64)>,
}


/// Find all UClass instances by scanning for objects with ClassPrivate == UCLASS_METACLASS_ADDR
/// This is more reliable than walking GUObjectArray when the array location is uncertain
pub fn find_all_uclasses(
    source: &dyn MemorySource,
    fname_reader: &mut FNameReader,
) -> Result<Vec<UObjectInfo>> {
    let code_bounds = find_code_bounds(source)?;
    let mut results = Vec::new();
    let mut scanned_bytes = 0usize;

    eprintln!(
        "Scanning for UClass instances (ClassPrivate == {:#x})...",
        UCLASS_METACLASS_ADDR
    );

    // Scan all readable regions in the executable's data space
    for region in source.regions() {
        if !region.is_readable() {
            continue;
        }

        // Focus on PE + heap regions where UObjects live
        let in_pe = region.start >= 0x140000000 && region.start <= 0x175000000;
        let in_heap = region.start >= 0x1000000 && region.start < 0x140000000;
        if !in_pe && !in_heap {
            continue;
        }

        // Skip very large regions (heap can be huge)
        if region.size() > 100 * 1024 * 1024 {
            continue;
        }

        let data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        scanned_bytes += data.len();

        // Scan for 8-byte aligned pointers to the UClass metaclass
        for i in (0..data.len().saturating_sub(UOBJECT_HEADER_SIZE)).step_by(8) {
            // Check ClassPrivate at offset 0x18
            if i + UOBJECT_CLASS_OFFSET + 8 > data.len() {
                continue;
            }

            let class_ptr =
                LE::read_u64(&data[i + UOBJECT_CLASS_OFFSET..i + UOBJECT_CLASS_OFFSET + 8])
                    as usize;

            if class_ptr != UCLASS_METACLASS_ADDR {
                continue;
            }

            let obj_addr = region.start + i;

            // Validate vtable
            let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;
            if vtable_ptr < MIN_VTABLE_ADDR || vtable_ptr > MAX_VTABLE_ADDR {
                continue;
            }

            // Verify vtable[0] points to code
            if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                let first_func = LE::read_u64(&vtable_data) as usize;
                if !code_bounds.contains(first_func) {
                    continue;
                }
            } else {
                continue;
            }

            // Read FName
            let name_index =
                LE::read_u32(&data[i + UOBJECT_NAME_OFFSET..i + UOBJECT_NAME_OFFSET + 4]);

            // Resolve name
            let name = match fname_reader.read_name(source, name_index) {
                Ok(n) => n,
                Err(_) => format!("FName_{}", name_index),
            };

            results.push(UObjectInfo {
                address: obj_addr,
                class_ptr,
                name_index,
                name,
                class_name: "Class".to_string(),
            });
        }
    }

    eprintln!(
        "Scanned {} MB, found {} UClass instances",
        scanned_bytes / 1_000_000,
        results.len()
    );

    // Sort by name for easier reading
    results.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(results)
}

/// UE5 UObject offsets
/// These vary by engine version but are consistent within a build
pub struct UObjectOffsets {
    /// Offset of ClassPrivate (UClass*) in UObject
    pub class_offset: usize,
    /// Offset of NamePrivate (FName) in UObject
    pub name_offset: usize,
    /// Offset of OuterPrivate (UObject*) in UObject
    pub outer_offset: usize,
}

impl Default for UObjectOffsets {
    fn default() -> Self {
        // Uses constants defined at top of file - see UOBJECT_* for documentation
        Self {
            class_offset: UOBJECT_CLASS_OFFSET,
            name_offset: UOBJECT_NAME_OFFSET,
            outer_offset: UOBJECT_OUTER_OFFSET,
        }
    }
}

/// Result of UClass metaclass discovery
#[derive(Debug, Clone)]
pub struct UClassMetaclassInfo {
    /// Address of the UClass metaclass
    pub address: usize,
    /// Vtable address
    pub vtable: usize,
    /// Offset where ClassPrivate was found
    pub class_offset: usize,
    /// Offset where NamePrivate was found
    pub name_offset: usize,
    /// FName index
    pub fname_index: u32,
    /// Resolved name
    pub name: String,
}

/// Find the UClass metaclass by exhaustively searching for self-referential objects
/// with FName "Class" (index 588) at various layout offsets.
///
/// This function tries different combinations of ClassPrivate and NamePrivate offsets
/// to handle UE5 version differences.
pub fn discover_uclass_metaclass_exhaustive(
    source: &dyn MemorySource,
    fname_reader: &mut FNameReader,
) -> Result<UClassMetaclassInfo> {
    let code_bounds = find_code_bounds(source)?;

    eprintln!("=== Exhaustive UClass Metaclass Discovery ===");

    // Find the actual FName index for "Class" dynamically
    let class_fname_idx = fname_reader
        .find_class_index(source)
        .unwrap_or(FNAME_CLASS_INDEX);
    eprintln!(
        "Looking for self-referential object with FName 'Class' ({})...",
        class_fname_idx
    );

    // Possible offsets to try
    let class_offsets = [0x08, 0x10, 0x18, 0x20, 0x28];
    let name_offsets = [0x18, 0x20, 0x28, 0x30, 0x38, 0x40];

    // Build the 4-byte pattern for the Class FName index (little-endian)
    let class_fname_bytes = class_fname_idx.to_le_bytes();

    // Scan ALL readable memory for objects with FName "Class"
    eprintln!(
        "Scanning all memory for objects with FName {} ('Class')...",
        class_fname_idx
    );

    let mut scanned_mb = 0usize;
    for region in source.regions() {
        if !region.is_readable() {
            continue;
        }

        // Scan in chunks for large regions
        let chunk_size = 256 * 1024 * 1024; // 256MB chunks
        let mut offset = 0usize;

        while offset < region.size() {
            let read_size = (region.size() - offset).min(chunk_size);
            let chunk_start = region.start + offset;

            let data = match source.read_bytes(chunk_start, read_size) {
                Ok(d) => d,
                Err(_) => {
                    offset += chunk_size;
                    continue;
                }
            };

            scanned_mb += data.len() / (1024 * 1024);
            if scanned_mb % 1000 == 0 && scanned_mb > 0 {
                eprint!("\r  Scanned {} MB...", scanned_mb);
            }

            // Boyer-Moore style: search for the FName index bytes first
            let mut pos = 0;
            while pos + 64 < data.len() {
                // Try each name_offset to find the FName pattern
                for &name_offset in &name_offsets {
                    if pos + name_offset + 4 > data.len() {
                        continue;
                    }

                    // Check if FName at this offset matches
                    if data[pos + name_offset..pos + name_offset + 4] != class_fname_bytes[..] {
                        continue;
                    }

                    // Found potential match - validate structure
                    for &class_offset in &class_offsets {
                        if class_offset == name_offset {
                            continue;
                        }

                        let max_offset = class_offset.max(name_offset) + 8;
                        if pos + max_offset > data.len() {
                            continue;
                        }

                        let obj_addr = chunk_start + pos;

                        // Check vtable
                        let vtable_ptr = LE::read_u64(&data[pos..pos + 8]) as usize;
                        if vtable_ptr < MIN_VTABLE_ADDR || vtable_ptr > MAX_VTABLE_ADDR {
                            continue;
                        }

                        // Verify vtable[0] points to code
                        if let Ok(vtable_data) = source.read_bytes(vtable_ptr, 8) {
                            let first_func = LE::read_u64(&vtable_data) as usize;
                            if !code_bounds.contains(first_func) {
                                continue;
                            }
                        } else {
                            continue;
                        }

                        // Check if ClassPrivate == self (self-referential)
                        let class_ptr =
                            LE::read_u64(&data[pos + class_offset..pos + class_offset + 8])
                                as usize;
                        if class_ptr == obj_addr {
                            eprintln!("\rFound UClass metaclass at {:#x}!", obj_addr);
                            eprintln!("  vtable: {:#x}", vtable_ptr);
                            eprintln!("  ClassPrivate offset: {:#x}", class_offset);
                            eprintln!("  NamePrivate offset: {:#x}", name_offset);

                            let fname_idx =
                                LE::read_u32(&data[pos + name_offset..pos + name_offset + 4]);
                            let name = fname_reader
                                .read_name(source, fname_idx)
                                .unwrap_or_else(|_| format!("FName_{}", fname_idx));

                            return Ok(UClassMetaclassInfo {
                                address: obj_addr,
                                vtable: vtable_ptr,
                                class_offset,
                                name_offset,
                                fname_index: fname_idx,
                                name,
                            });
                        }
                    }
                }
                pos += 8; // Align to 8-byte boundary
            }

            offset += chunk_size;
        }
    }
    eprintln!("\r  Scanned {} MB total", scanned_mb);

    // Second approach: find any self-referential object and check its FName
    eprintln!(
        "\nNo self-referential object with FName {} found.",
        class_fname_idx
    );
    eprintln!("Searching all memory for any self-referential objects...");

    let mut self_refs: Vec<(usize, usize, usize, usize, u32, String)> = Vec::new();
    let mut scanned_mb2 = 0usize;

    'outer: for region in source.regions() {
        if !region.is_readable() {
            continue;
        }

        // Scan in chunks for large regions
        let chunk_size = 256 * 1024 * 1024;
        let mut offset = 0usize;

        while offset < region.size() && self_refs.len() < 50 {
            let read_size = (region.size() - offset).min(chunk_size);
            let chunk_start = region.start + offset;

            let data = match source.read_bytes(chunk_start, read_size) {
                Ok(d) => d,
                Err(_) => {
                    offset += chunk_size;
                    continue;
                }
            };

            scanned_mb2 += data.len() / (1024 * 1024);
            if scanned_mb2 % 2000 == 0 && scanned_mb2 > 0 {
                eprint!("\r  Scanned {} MB for self-refs...", scanned_mb2);
            }

            for &class_offset in &class_offsets {
                for &name_offset in &name_offsets {
                    if class_offset == name_offset {
                        continue;
                    }

                    let max_offset = class_offset.max(name_offset) + 8;

                    for i in (0..data.len().saturating_sub(max_offset)).step_by(8) {
                        let obj_addr = chunk_start + i;

                        // Check ClassPrivate == self first (fast filter)
                        let class_ptr =
                            LE::read_u64(&data[i + class_offset..i + class_offset + 8]) as usize;
                        if class_ptr != obj_addr {
                            continue;
                        }

                        // Validate vtable
                        let vtable_ptr = LE::read_u64(&data[i..i + 8]) as usize;
                        if vtable_ptr < MIN_VTABLE_ADDR || vtable_ptr > MAX_VTABLE_ADDR {
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

                        // Read FName
                        let fname_idx = LE::read_u32(&data[i + name_offset..i + name_offset + 4]);
                        let name = fname_reader
                            .read_name(source, fname_idx)
                            .unwrap_or_else(|_| format!("FName_{}", fname_idx));

                        self_refs.push((
                            obj_addr,
                            vtable_ptr,
                            class_offset,
                            name_offset,
                            fname_idx,
                            name,
                        ));

                        if self_refs.len() >= 50 {
                            break 'outer;
                        }
                    }
                }
            }

            offset += chunk_size;
        }
    }
    eprintln!("\r  Scanned {} MB for self-refs", scanned_mb2);

    eprintln!(
        "Found {} self-referential objects with valid vtables:",
        self_refs.len()
    );
    for (addr, vt, cls_off, name_off, idx, name) in &self_refs {
        let marker = if *idx == class_fname_idx || name == "Class" {
            " <-- METACLASS!"
        } else {
            ""
        };
        eprintln!(
            "  {:#x}: vt={:#x}, cls@+{:#x}, name@+{:#x}, FName={}(\"{}\"){}",
            addr, vt, cls_off, name_off, idx, name, marker
        );
    }

    // Check if any is "Class"
    if let Some((addr, vt, cls_off, name_off, idx, name)) = self_refs
        .iter()
        .find(|(_, _, _, _, idx, name)| *idx == class_fname_idx || name == "Class")
    {
        return Ok(UClassMetaclassInfo {
            address: *addr,
            vtable: *vt,
            class_offset: *cls_off,
            name_offset: *name_off,
            fname_index: *idx,
            name: name.clone(),
        });
    }

    bail!("UClass metaclass not found in dump. The dump may be incomplete or the FName format is different.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_type_from_name() {
        assert_eq!(EPropertyType::from_name("IntProperty"), EPropertyType::IntProperty);
        assert_eq!(EPropertyType::from_name("BoolProperty"), EPropertyType::BoolProperty);
        assert_eq!(EPropertyType::from_name("StructProperty"), EPropertyType::StructProperty);
        assert_eq!(EPropertyType::from_name("ArrayProperty"), EPropertyType::ArrayProperty);
        assert_eq!(EPropertyType::from_name("MapProperty"), EPropertyType::MapProperty);
        assert_eq!(EPropertyType::from_name("UnknownType"), EPropertyType::Unknown);
    }

    #[test]
    fn test_property_type_aliases() {
        // ClassProperty maps to ObjectProperty
        assert_eq!(EPropertyType::from_name("ClassProperty"), EPropertyType::ObjectProperty);
        // SoftClassProperty maps to SoftObjectProperty
        assert_eq!(EPropertyType::from_name("SoftClassProperty"), EPropertyType::SoftObjectProperty);
        // MulticastInlineDelegateProperty maps to MulticastDelegateProperty
        assert_eq!(
            EPropertyType::from_name("MulticastInlineDelegateProperty"),
            EPropertyType::MulticastDelegateProperty
        );
    }

    #[test]
    fn test_property_type_to_usmap_id() {
        assert_eq!(EPropertyType::ByteProperty.to_usmap_id(), 0);
        assert_eq!(EPropertyType::BoolProperty.to_usmap_id(), 1);
        assert_eq!(EPropertyType::IntProperty.to_usmap_id(), 2);
        assert_eq!(EPropertyType::StructProperty.to_usmap_id(), 9);
        assert_eq!(EPropertyType::MapProperty.to_usmap_id(), 24);
        assert_eq!(EPropertyType::Unknown.to_usmap_id(), 255);
    }

    #[test]
    fn test_property_type_roundtrip() {
        let types = [
            "ByteProperty", "BoolProperty", "IntProperty", "FloatProperty",
            "ObjectProperty", "NameProperty", "StructProperty", "ArrayProperty",
        ];
        for name in types {
            let prop_type = EPropertyType::from_name(name);
            assert_ne!(prop_type, EPropertyType::Unknown, "Failed for {}", name);
            assert_ne!(prop_type.to_usmap_id(), 255, "Failed usmap id for {}", name);
        }
    }
}
