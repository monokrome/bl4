//! USMAP file command handlers
//!
//! Handlers for inspecting and searching USMAP reflection files.

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian as LE, ReadBytesExt};
use std::fs;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Property type names for display
const PROPERTY_TYPE_NAMES: &[&str] = &[
    "Byte",
    "Bool",
    "Int",
    "Float",
    "Object",
    "Name",
    "Delegate",
    "Double",
    "Array",
    "Struct",
    "Str",
    "Text",
    "Interface",
    "MulticastDelegate",
    "WeakObject",
    "LazyObject",
    "AssetObject",
    "SoftObject",
    "UInt64",
    "UInt32",
    "UInt16",
    "Int64",
    "Int16",
    "Int8",
    "Map",
    "Set",
    "Enum",
    "FieldPath",
    "Optional",
    "Utf8Str",
    "AnsiStr",
];

/// Handle the Usmap Info command
///
/// Displays header information and statistics from a USMAP file.
pub fn handle_info(path: &Path) -> Result<()> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);

    // Read header
    let magic = reader.read_u16::<LE>()?;
    if magic != 0x30C4 {
        bail!("Invalid usmap magic: expected 0x30C4, got {:#x}", magic);
    }

    let version = reader.read_u8()?;
    let has_version_info = if version >= 1 {
        reader.read_u8()? != 0
    } else {
        false
    };

    let compression = reader.read_u32::<LE>()?;
    let compressed_size = reader.read_u32::<LE>()?;
    let decompressed_size = reader.read_u32::<LE>()?;

    println!("=== {} ===", path.display());
    println!("Magic: {:#x}", magic);
    println!("Version: {}", version);
    println!("HasVersionInfo: {}", has_version_info);
    println!(
        "Compression: {} ({})",
        compression,
        match compression {
            0 => "None",
            1 => "Oodle",
            2 => "Brotli",
            3 => "ZStandard",
            _ => "Unknown",
        }
    );
    println!("CompressedSize: {} bytes", compressed_size);
    println!("DecompressedSize: {} bytes", decompressed_size);

    if compression != 0 {
        println!("\n(Compressed payloads not yet supported for detailed analysis)");
    } else {
        // Read payload
        let name_count = reader.read_u32::<LE>()?;
        println!("\nNames: {}", name_count);

        // Skip names
        for _ in 0..name_count {
            let len = reader.read_u16::<LE>()? as usize;
            reader.seek(SeekFrom::Current(len as i64))?;
        }

        let enum_count = reader.read_u32::<LE>()?;
        println!("Enums: {}", enum_count);

        // Count enum values
        let mut total_enum_values = 0u64;
        for _ in 0..enum_count {
            let _name_idx = reader.read_u32::<LE>()?;
            let entry_count = reader.read_u16::<LE>()? as u64;
            total_enum_values += entry_count;
            // Version >= 4 uses ExplicitEnumValues (value u64 + name_idx u32 = 12 bytes)
            // Version 3 uses just name indices (4 bytes each)
            let bytes_per_entry = if version >= 4 { 12 } else { 4 };
            reader.seek(SeekFrom::Current((entry_count * bytes_per_entry) as i64))?;
        }
        println!("Enum values: {}", total_enum_values);

        let struct_count = reader.read_u32::<LE>()?;
        println!("Structs: {}", struct_count);

        // Count properties
        let mut total_props = 0u64;
        for _ in 0..struct_count {
            let _name_idx = reader.read_u32::<LE>()?;
            let _super_idx = reader.read_u32::<LE>()?;
            let _prop_count = reader.read_u16::<LE>()?;
            let serializable_count = reader.read_u16::<LE>()? as u64;
            total_props += serializable_count;

            // Skip properties (need to parse each one due to variable size)
            for _ in 0..serializable_count {
                let _index = reader.read_u16::<LE>()?;
                let _array_dim = reader.read_u8()?;
                let _name_idx = reader.read_u32::<LE>()?;
                // Read property type recursively
                skip_property_type(&mut reader)?;
            }
        }
        println!("Properties: {}", total_props);
    }

    let file_size = fs::metadata(path)?.len();
    println!("\nFile size: {} bytes", file_size);

    Ok(())
}

/// Skip over a property type in a USMAP file (for counting/seeking)
fn skip_property_type<R: std::io::Read>(r: &mut R) -> Result<()> {
    let type_id = r.read_u8()?;
    match type_id {
        26 => {
            // EnumProperty
            skip_property_type(r)?; // inner
            r.read_u32::<LE>()?; // enum name
        }
        9 => {
            // StructProperty
            r.read_u32::<LE>()?; // struct name
        }
        8 | 25 | 28 => {
            // Array/Set/Optional
            skip_property_type(r)?; // inner
        }
        24 => {
            // MapProperty
            skip_property_type(r)?; // key
            skip_property_type(r)?; // value
        }
        _ => {} // Simple types have no extra data
    }
    Ok(())
}

/// Read a property type and return its string representation
fn read_property_type<R: std::io::Read>(r: &mut R, names: &[String]) -> Result<String> {
    let type_id = r.read_u8()? as usize;
    let base_type = PROPERTY_TYPE_NAMES.get(type_id).unwrap_or(&"Unknown");

    Ok(match type_id {
        26 => {
            // EnumProperty
            let _inner = read_property_type(r, names)?;
            let enum_idx = r.read_u32::<LE>()? as usize;
            let enum_name = names.get(enum_idx).cloned().unwrap_or_default();
            format!("Enum<{}>", enum_name)
        }
        9 => {
            // StructProperty
            let struct_idx = r.read_u32::<LE>()? as usize;
            let struct_name = names.get(struct_idx).cloned().unwrap_or_default();
            format!("Struct<{}>", struct_name)
        }
        8 => {
            // ArrayProperty
            let inner = read_property_type(r, names)?;
            format!("Array<{}>", inner)
        }
        25 => {
            // SetProperty
            let inner = read_property_type(r, names)?;
            format!("Set<{}>", inner)
        }
        28 => {
            // OptionalProperty
            let inner = read_property_type(r, names)?;
            format!("Optional<{}>", inner)
        }
        24 => {
            // MapProperty
            let key = read_property_type(r, names)?;
            let value = read_property_type(r, names)?;
            format!("Map<{}, {}>", key, value)
        }
        _ => base_type.to_string(),
    })
}

/// Handle the Usmap Search command
///
/// Searches for enums and structs matching a pattern.
pub fn handle_search(path: &Path, pattern: &str, verbose: bool) -> Result<()> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);

    // Read header
    let magic = reader.read_u16::<LE>()?;
    if magic != 0x30C4 {
        bail!("Invalid usmap magic: expected 0x30C4, got {:#x}", magic);
    }

    let version = reader.read_u8()?;
    let _has_version_info = if version >= 1 {
        reader.read_u8()? != 0
    } else {
        false
    };

    let compression = reader.read_u32::<LE>()?;
    let _compressed_size = reader.read_u32::<LE>()?;
    let _decompressed_size = reader.read_u32::<LE>()?;

    if compression != 0 {
        bail!("Compressed usmap files not yet supported for search");
    }

    // Read names table
    let name_count = reader.read_u32::<LE>()?;
    let mut names: Vec<String> = Vec::with_capacity(name_count as usize);
    for _ in 0..name_count {
        let len = reader.read_u16::<LE>()? as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        names.push(String::from_utf8_lossy(&buf).into_owned());
    }

    // Read enums
    let enum_count = reader.read_u32::<LE>()?;
    let pattern_lower = pattern.to_lowercase();
    let mut found_enums = Vec::new();

    for _ in 0..enum_count {
        let name_idx = reader.read_u32::<LE>()? as usize;
        let entry_count = reader.read_u16::<LE>()? as usize;

        let name = names.get(name_idx).cloned().unwrap_or_default();
        if name.to_lowercase().contains(&pattern_lower) {
            let mut entries = Vec::new();
            for _ in 0..entry_count {
                let entry_idx = reader.read_u32::<LE>()? as usize;
                entries.push(names.get(entry_idx).cloned().unwrap_or_default());
            }
            found_enums.push((name, entries));
        } else {
            // Skip entries
            reader.seek(SeekFrom::Current((entry_count * 4) as i64))?;
        }
    }

    // Read structs
    let struct_count = reader.read_u32::<LE>()?;
    let mut found_structs = Vec::new();

    for _ in 0..struct_count {
        let name_idx = reader.read_u32::<LE>()? as usize;
        let super_idx = reader.read_u32::<LE>()? as usize;
        let _prop_count = reader.read_u16::<LE>()?;
        let serializable_count = reader.read_u16::<LE>()? as usize;

        let name = names.get(name_idx).cloned().unwrap_or_default();
        let super_name = if super_idx == 0xFFFFFFFF {
            None
        } else {
            names.get(super_idx).cloned()
        };

        // Read properties
        let mut properties = Vec::new();
        for _ in 0..serializable_count {
            let _index = reader.read_u16::<LE>()?;
            let array_dim = reader.read_u8()?;
            let prop_name_idx = reader.read_u32::<LE>()? as usize;
            let prop_name = names.get(prop_name_idx).cloned().unwrap_or_default();
            let prop_type = read_property_type(&mut reader, &names)?;

            properties.push((prop_name, prop_type, array_dim));
        }

        if name.to_lowercase().contains(&pattern_lower) {
            found_structs.push((name, super_name, properties));
        }
    }

    // Print results
    if !found_enums.is_empty() {
        println!(
            "=== Enums matching '{}' ({}) ===",
            pattern,
            found_enums.len()
        );
        for (name, entries) in &found_enums {
            println!("\n{} ({} values)", name, entries.len());
            if verbose {
                for (i, entry) in entries.iter().enumerate() {
                    println!("  {} = {}", i, entry);
                }
            }
        }
    }

    if !found_structs.is_empty() {
        println!(
            "\n=== Structs matching '{}' ({}) ===",
            pattern,
            found_structs.len()
        );
        for (name, super_name, properties) in &found_structs {
            println!(
                "\n{}{} ({} properties)",
                name,
                super_name
                    .as_ref()
                    .map(|s| format!(" : {}", s))
                    .unwrap_or_default(),
                properties.len()
            );
            if verbose {
                for (prop_name, prop_type, array_dim) in properties {
                    let dim_str = if *array_dim > 1 {
                        format!("[{}]", array_dim)
                    } else {
                        String::new()
                    };
                    println!("  {} {}{}", prop_type, prop_name, dim_str);
                }
            }
        }
    }

    if found_enums.is_empty() && found_structs.is_empty() {
        println!("No enums or structs found matching '{}'", pattern);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_type_names_count() {
        // Ensure we have all property types defined
        assert!(PROPERTY_TYPE_NAMES.len() >= 30);
    }

    #[test]
    fn test_handle_info_missing_file() {
        let result = handle_info(Path::new("/nonexistent/file.usmap"));
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_search_missing_file() {
        let result = handle_search(Path::new("/nonexistent/file.usmap"), "test", false);
        assert!(result.is_err());
    }
}
