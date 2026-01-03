//! PE binary parsing and pattern scanning
//!
//! Provides:
//! - PE header parsing to identify code sections
//! - Code bounds detection for vtable validation
//! - Memory pattern scanning

use super::constants::*;
use super::pattern::scan_pattern_fast;
use super::source::MemorySource;

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};

/// PE executable section information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PeSection {
    pub name: String,
    pub virtual_address: usize,
    pub virtual_size: usize,
    pub characteristics: u32,
}

impl PeSection {
    /// Check if this section is executable (contains code)
    pub fn is_executable(&self) -> bool {
        // IMAGE_SCN_MEM_EXECUTE = 0x20000000
        // IMAGE_SCN_CNT_CODE = 0x00000020
        (self.characteristics & 0x20000020) != 0
    }
}

/// Code section bounds for vtable validation
/// Holds multiple ranges since there can be gaps between code sections
#[derive(Debug, Clone)]
pub struct CodeBounds {
    pub ranges: Vec<(usize, usize)>, // (start, end) pairs
}

impl CodeBounds {
    /// Check if an address is within any code section
    pub fn contains(&self, addr: usize) -> bool {
        self.ranges
            .iter()
            .any(|(start, end)| addr >= *start && addr < *end)
    }
}

/// Parse PE header to find code section bounds
/// Works with both live processes and memory dumps
pub fn find_code_bounds(source: &dyn MemorySource) -> Result<CodeBounds> {
    // Find PE image base by looking for MZ header in typical locations
    let pe_bases = [
        0x140000000usize, // Windows x64 default image base
        0x400000,         // Windows x86 default
        0x10000,          // Alternative
    ];

    for &base in &pe_bases {
        if let Ok(bounds) = parse_pe_code_section(source, base) {
            for (start, end) in &bounds.ranges {
                eprintln!(
                    "Found code range: {:#x}-{:#x} (from PE at {:#x})",
                    start, end, base
                );
            }
            return Ok(bounds);
        }
    }

    // Fallback: scan for MZ header in memory regions
    for region in source.regions() {
        if region.start < 0x100000 || region.size() < 0x1000 {
            continue;
        }

        // Check for MZ header
        if let Ok(header) = source.read_bytes(region.start, 2) {
            if header == b"MZ" {
                if let Ok(bounds) = parse_pe_code_section(source, region.start) {
                    for (start, end) in &bounds.ranges {
                        eprintln!(
                            "Found code range: {:#x}-{:#x} (from PE at {:#x})",
                            start, end, region.start
                        );
                    }
                    return Ok(bounds);
                }
            }
        }
    }

    // Ultimate fallback: use hardcoded values for BL4
    eprintln!("Warning: Could not parse PE header, using fallback code bounds");
    Ok(CodeBounds {
        ranges: vec![(0x140001000, 0x14f000000)], // Conservative range for .ecode only
    })
}

/// Parse PE header at given base address to find code section
fn parse_pe_code_section(source: &dyn MemorySource, base: usize) -> Result<CodeBounds> {
    // Read DOS header
    let dos_header = source.read_bytes(base, 64)?;

    // Check MZ signature
    if dos_header[0] != b'M' || dos_header[1] != b'Z' {
        bail!("Invalid DOS signature at {:#x}", base);
    }

    // Get PE header offset (e_lfanew)
    let pe_offset =
        LE::read_u32(&dos_header[PE_HEADER_OFFSET_LOCATION..PE_HEADER_OFFSET_LOCATION + 4])
            as usize;
    if pe_offset == 0 || pe_offset > PE_HEADER_MAX_OFFSET {
        bail!("Invalid PE offset: {:#x}", pe_offset);
    }

    // Read PE header
    let pe_header = source.read_bytes(base + pe_offset, 264)?; // PE sig + COFF header + Optional header

    // Check PE signature
    if &pe_header[0..4] != b"PE\0\0" {
        bail!("Invalid PE signature at {:#x}", base + pe_offset);
    }

    // Parse COFF header (starts at offset 4)
    let number_of_sections = LE::read_u16(&pe_header[6..8]) as usize;
    let size_of_optional_header = LE::read_u16(&pe_header[20..22]) as usize;

    if number_of_sections == 0 || number_of_sections > 100 {
        bail!("Invalid section count: {}", number_of_sections);
    }

    // Section headers start after optional header
    // COFF header is 20 bytes, optional header follows
    let sections_offset = pe_offset + 24 + size_of_optional_header;

    // Read all section headers (40 bytes each)
    let sections_data = source.read_bytes(base + sections_offset, number_of_sections * 40)?;

    let mut code_ranges: Vec<(usize, usize)> = Vec::new();

    for i in 0..number_of_sections {
        let section_offset = i * 40;
        let section_data = &sections_data[section_offset..section_offset + 40];

        // Section name (8 bytes, null-padded)
        let name_bytes = &section_data[0..8];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        // Virtual size, virtual address, characteristics
        let virtual_size = LE::read_u32(&section_data[8..12]) as usize;
        let virtual_address = LE::read_u32(&section_data[12..16]) as usize;
        let characteristics = LE::read_u32(&section_data[36..40]);

        let section = PeSection {
            name: name.clone(),
            virtual_address,
            virtual_size,
            characteristics,
        };

        // Check if this is an actual code section (not just executable metadata)
        // Only include sections that actually contain code, not .pdata/.reloc/etc
        let is_code_section = section.is_executable()
            && (name.contains("text")
                || name.contains("code")
                || name == ".ecode"
                || name.starts_with(".text")
                || name.starts_with(".code"));

        let section_start = base + virtual_address;
        let section_end = section_start + virtual_size;

        if is_code_section {
            code_ranges.push((section_start, section_end));
            eprintln!(
                "  Found code section '{}': {:#x}-{:#x}",
                name, section_start, section_end
            );
        } else if section.is_executable() {
            eprintln!(
                "  Skipping executable non-code '{}': {:#x}-{:#x}",
                name, section_start, section_end
            );
        } else {
            // Print non-executable sections for debugging (like .rdata where vtables live)
            if name.contains("data") || name.contains("rdata") {
                eprintln!(
                    "  Found data section '{}': {:#x}-{:#x} (chars: {:#x})",
                    name, section_start, section_end, characteristics
                );
            }
        }
    }

    if code_ranges.is_empty() {
        bail!("No code sections found in PE at {:#x}", base);
    }

    Ok(CodeBounds {
        ranges: code_ranges,
    })
}

/// Scan memory for a byte pattern with mask (SIMD-accelerated Boyer-Moore style)
pub fn scan_pattern(source: &dyn MemorySource, pattern: &[u8], mask: &[u8]) -> Result<Vec<usize>> {
    let mut results = Vec::new();

    for region in source.regions() {
        if !region.is_readable() || region.size() > 100 * 1024 * 1024 {
            continue; // Skip non-readable or huge regions
        }

        if let Ok(data) = source.read_bytes(region.start, region.size()) {
            // Use fast SIMD-accelerated pattern matching
            for offset in scan_pattern_fast(&data, pattern, mask) {
                results.push(region.start + offset);
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;
    use crate::memory::source::MemoryRegion;

    #[test]
    fn test_scan_pattern_finds_single_match() {
        // Create mock with a known pattern
        let mut data = vec![0u8; 100];
        data[50..54].copy_from_slice(b"TEST");

        let source = MockMemorySource::new(data, 0x1000);
        let pattern = b"TEST";
        let mask = vec![1u8; 4];

        let results = scan_pattern(&source, pattern, &mask).unwrap();
        assert_eq!(results, vec![0x1000 + 50]);
    }

    #[test]
    fn test_scan_pattern_finds_multiple_matches() {
        let mut data = vec![0u8; 100];
        data[10..14].copy_from_slice(b"FIND");
        data[50..54].copy_from_slice(b"FIND");
        data[90..94].copy_from_slice(b"FIND");

        let source = MockMemorySource::new(data, 0x2000);
        let pattern = b"FIND";
        let mask = vec![1u8; 4];

        let results = scan_pattern(&source, pattern, &mask).unwrap();
        assert_eq!(results, vec![0x2000 + 10, 0x2000 + 50, 0x2000 + 90]);
    }

    #[test]
    fn test_scan_pattern_with_wildcards() {
        let mut data = vec![0u8; 100];
        data[20..28].copy_from_slice(b"AB\x00\x00CD\x00\x00");
        data[60..68].copy_from_slice(b"AB\xff\xffCD\xaa\xbb");

        let source = MockMemorySource::new(data, 0x3000);
        // Pattern "AB??CD??" where ?? are wildcards
        let pattern = b"AB\x00\x00CD\x00\x00";
        let mask = vec![1, 1, 0, 0, 1, 1, 0, 0]; // 1=must match, 0=wildcard

        let results = scan_pattern(&source, pattern, &mask).unwrap();
        assert_eq!(results, vec![0x3000 + 20, 0x3000 + 60]);
    }

    #[test]
    fn test_scan_pattern_no_matches() {
        let data = vec![0u8; 100];
        let source = MockMemorySource::new(data, 0x1000);

        let pattern = b"NOTFOUND";
        let mask = vec![1u8; 8];

        let results = scan_pattern(&source, pattern, &mask).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_pattern_skips_non_readable_regions() {
        let data = vec![0u8; 100];
        let source = MockMemorySource::with_regions(
            data,
            0x1000,
            vec![MemoryRegion {
                start: 0x1000,
                end: 0x1064,
                perms: "-w-p".to_string(), // Not readable
                offset: 0,
                path: None,
            }],
        );

        let pattern = b"TEST";
        let mask = vec![1u8; 4];

        let results = scan_pattern(&source, pattern, &mask).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_code_bounds_contains() {
        let bounds = CodeBounds {
            ranges: vec![(0x1000, 0x2000), (0x3000, 0x4000)],
        };

        // In first range
        assert!(bounds.contains(0x1000));
        assert!(bounds.contains(0x1500));
        assert!(bounds.contains(0x1fff));

        // Between ranges
        assert!(!bounds.contains(0x2000));
        assert!(!bounds.contains(0x2500));

        // In second range
        assert!(bounds.contains(0x3000));
        assert!(bounds.contains(0x3fff));

        // Outside all ranges
        assert!(!bounds.contains(0x500));
        assert!(!bounds.contains(0x5000));
    }

    #[test]
    fn test_pe_section_is_executable() {
        // IMAGE_SCN_MEM_EXECUTE = 0x20000000
        let executable = PeSection {
            name: ".text".to_string(),
            virtual_address: 0x1000,
            virtual_size: 0x1000,
            characteristics: 0x20000020, // EXECUTE | CODE
        };
        assert!(executable.is_executable());

        let data_section = PeSection {
            name: ".data".to_string(),
            virtual_address: 0x2000,
            virtual_size: 0x1000,
            characteristics: 0xC0000040, // READ | WRITE | INITIALIZED_DATA
        };
        assert!(!data_section.is_executable());
    }

    #[test]
    fn test_scan_pattern_gnames_signature() {
        // Simulate finding UE5 GNames pool signature
        // "None" followed by "ByteProperty" with FNameEntry headers
        let mut data = vec![0u8; 200];

        // FNameEntry for "None": length byte (4 + flags), then "None"
        // UE5 format: low 6 bits = length, upper bits = flags
        let none_offset = 50;
        data[none_offset] = 0x1e; // Header byte (length=4 with flags)
        data[none_offset + 1] = 0x01; // Additional flags
        data[none_offset + 2..none_offset + 6].copy_from_slice(b"None");

        // FNameEntry for "ByteProperty"
        data[none_offset + 6] = 0x10; // Header
        data[none_offset + 7] = 0x03; // Flags
        data[none_offset + 8..none_offset + 20].copy_from_slice(b"ByteProperty");

        let source = MockMemorySource::new(data, 0x150000000);

        // Search for the GNames signature pattern
        let pattern = b"\x1e\x01None\x10\x03ByteProperty";
        let mask = vec![1u8; pattern.len()];

        let results = scan_pattern(&source, pattern, &mask).unwrap();
        assert_eq!(results, vec![0x150000000 + none_offset]);
    }
}
