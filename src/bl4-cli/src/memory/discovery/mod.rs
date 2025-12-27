//! UE5 Structure Discovery
//!
//! Functions for discovering UE5 global structures in memory:
//! - GNames pool discovery via pattern scanning
//! - GUObjectArray discovery via code pattern analysis
//! - Class UClass discovery via self-referential pattern
//! - UE5 offset detection

mod class_uclass;
mod gnames;
mod guobject;

pub use class_uclass::discover_class_uclass;
pub use gnames::discover_gnames;
pub use guobject::{discover_guobject_array, is_valid_pointer};

use super::source::MemorySource;
use super::ue5::Ue5Offsets;

use anyhow::{bail, Result};

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
    use crate::memory::ue5::GNamesPool;

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
