//! NCS (Nexus Config Store) parser for Borderlands 4
//!
//! NCS files are Oodle-compressed configuration stores used by the game.
//!
//! # Format Overview
//!
//! ## NCS Data Format (`[version]NCS`)
//!
//! Compressed configuration data:
//! - Byte 0: Version byte (typically 0x01)
//! - Bytes 1-3: "NCS" magic
//! - Bytes 4-7: Compression flag
//! - Bytes 8-11: Decompressed size
//! - Bytes 12-15: Compressed size
//! - Bytes 16+: Payload
//!
//! ## NCS Manifest Format (`_NCS/`)
//!
//! Index files listing NCS data stores:
//! - Bytes 0-4: "_NCS/" magic
//! - Bytes 6-7: Entry count
//! - Remaining: Metadata and string table

mod bit_reader;
mod content;
mod data;
pub mod drops;
mod extract;
mod field;
mod hash;
mod header;
mod manifest;
pub mod name_data;
pub mod oodle;
pub mod pak;
mod parser;
mod types;

// Re-export main types
pub use content::{Content as NcsContent, Header as NcsContentHeader};
pub use drops::{
    extract_drops_from_itempool, extract_drops_from_itempoollist, generate_drops_manifest,
    DropEntry, DropLocation, DropProbabilities, DropSource, DropsDb, DropsManifest,
};
pub use data::{
    decompress as decompress_ncs, decompress_with as decompress_ncs_with, scan as scan_for_ncs,
    Header as NcsHeader,
};
pub use extract::{extract_from_pak, ExtractionResult, NcsFile};
pub use field::{known as fields, Field, Type as FieldType};
pub use hash::fnv1a_hash;
pub use manifest::{
    scan as scan_for_ncs_manifests, Entry as NcsManifestEntry, Manifest as NcsManifest,
};
pub use name_data::{
    extract_from_directory as extract_name_data, NameDataEntry, NameDataMap,
};
pub use pak::{
    extract_all as extract_all_ncs, type_from_filename, DirectoryReader, ExtractedNcs,
    MemoryPakReader, NcsReader, PakReader,
};
pub use bit_reader::{bit_width, BitReader};
pub use parser::{
    create_combined_string_table, debug_binary_section, extract_field_abbreviation,
    extract_inline_strings, find_packed_strings, parse_binary_section, parse_document,
    parse_header, parse_string_table, unpack_string,
};
pub use types::{
    BinaryEntry, BinaryParseResult, BinaryRecord, Document, EntryGroup, FieldInfo,
    Header as ParsedHeader, Record, StringTable, TagType, TagValue, UnpackedString, UnpackedValue,
    Value,
};

/// Magic bytes for NCS format: "NCS" (bytes 1-3 of header)
pub const NCS_MAGIC: [u8; 3] = [0x4e, 0x43, 0x53];

/// Magic bytes for NCS manifest format: "_NCS/"
pub const NCS_MANIFEST_MAGIC: [u8; 5] = [0x5f, 0x4e, 0x43, 0x53, 0x2f];

/// Inner compressed data magic (big-endian)
pub const OODLE_MAGIC: u32 = 0xb7756362;

/// Header size in bytes
pub const NCS_HEADER_SIZE: usize = data::HEADER_SIZE;

/// Manifest header size
pub const NCS_MANIFEST_HEADER_SIZE: usize = manifest::HEADER_SIZE;

/// Inner header minimum size
pub const NCS_INNER_HEADER_MIN: usize = data::INNER_HEADER_MIN;

/// Errors from NCS parsing
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid NCS magic: expected 'NCS', got {0:02x} {1:02x} {2:02x}")]
    InvalidNcsMagic(u8, u8, u8),

    #[error("Invalid NCS manifest magic: expected '_NCS/', got {0:?}")]
    InvalidManifestMagic([u8; 5]),

    #[error("Invalid inner magic: expected 0xb7756362, got 0x{0:08x}")]
    InvalidInnerMagic(u32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Oodle decompression error: {0}")]
    Oodle(String),

    #[error("Decompression size mismatch: expected {expected}, got {actual}")]
    DecompressionSize { expected: usize, actual: usize },

    #[error("Data too short: need {needed} bytes, got {actual}")]
    DataTooShort { needed: usize, actual: usize },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Check if data starts with NCS data magic
pub fn is_ncs(data: &[u8]) -> bool {
    data.len() >= 4 && data[1..4] == NCS_MAGIC && data[0] != b'_'
}

/// Check if data starts with NCS manifest magic
pub fn is_ncs_manifest(data: &[u8]) -> bool {
    data.len() >= 5 && data[0..5] == NCS_MANIFEST_MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ncs() {
        // Valid NCS: version byte + "NCS"
        assert!(is_ncs(&[0x01, 0x4e, 0x43, 0x53, 0x00]));

        // Invalid: "_NCS/" manifest format
        assert!(!is_ncs(&[0x5f, 0x4e, 0x43, 0x53, 0x2f]));

        // Too short
        assert!(!is_ncs(&[0x01, 0x4e, 0x43]));
    }

    #[test]
    fn test_is_ncs_manifest() {
        assert!(is_ncs_manifest(&[0x5f, 0x4e, 0x43, 0x53, 0x2f, 0x00]));
        assert!(!is_ncs_manifest(&[0x01, 0x4e, 0x43, 0x53, 0x00]));
    }

    #[test]
    fn test_magic_constants() {
        assert_eq!(NCS_MAGIC, *b"NCS");
        assert_eq!(NCS_MANIFEST_MAGIC, *b"_NCS/");
        assert_eq!(OODLE_MAGIC, 0xb7756362);
    }

    #[test]
    fn test_header_size_constants() {
        assert_eq!(NCS_HEADER_SIZE, 16);
        assert_eq!(NCS_MANIFEST_HEADER_SIZE, 8);
        assert_eq!(NCS_INNER_HEADER_MIN, 0x40);
    }

    #[test]
    fn test_error_display() {
        let err = Error::InvalidNcsMagic(0x00, 0x00, 0x00);
        assert!(err.to_string().contains("Invalid NCS magic"));

        let err = Error::InvalidManifestMagic([0x00; 5]);
        assert!(err.to_string().contains("Invalid NCS manifest magic"));

        let err = Error::InvalidInnerMagic(0x00000000);
        assert!(err.to_string().contains("Invalid inner magic"));

        let err = Error::Oodle("test error".to_string());
        assert!(err.to_string().contains("Oodle decompression error"));

        let err = Error::DecompressionSize {
            expected: 100,
            actual: 50,
        };
        assert!(err.to_string().contains("Decompression size mismatch"));

        let err = Error::DataTooShort {
            needed: 16,
            actual: 8,
        };
        assert!(err.to_string().contains("Data too short"));
    }

    #[test]
    fn test_error_debug() {
        let err = Error::InvalidNcsMagic(0x00, 0x00, 0x00);
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidNcsMagic"));
    }
}

#[cfg(test)]
mod investigate_failures {
    use crate::data::{decompress, scan};
    use std::path::Path;

    #[test]
    fn find_v1_failures() {
        let paks_dir = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks");

        if let Ok(entries) = std::fs::read_dir(paks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "pak") {
                    let pak_name = path.file_name().unwrap().to_string_lossy();
                    let data = match std::fs::read(&path) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                    let chunks = scan(&data);
                    for (offset, header) in &chunks {
                        if let Err(e) = decompress(&data[*offset..]) {
                            println!("{} offset {}: {:?}", pak_name, offset, e);
                            println!("  Header: comp={} decomp={}", header.compressed_size, header.decompressed_size);
                            // Show first 64 bytes
                            let preview: String = data[*offset..*offset+64.min(data.len()-*offset)]
                                .iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
                            println!("  Bytes: {}", preview);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod parse_real_ncs {
    use crate::data::{decompress, scan};
    use crate::content::Content;
    use std::path::Path;

    #[test]
    fn parse_first_10_ncs() {
        let pak_path = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");
        
        if !pak_path.exists() {
            println!("Pak file not found, skipping test");
            return;
        }

        let data = std::fs::read(pak_path).expect("read pak");
        let chunks = scan(&data);
        
        println!("\nFound {} NCS chunks", chunks.len());
        
        let mut success = 0;
        let mut failed_parse = 0;
        
        for (offset, header) in chunks.iter().take(10) {
            println!("\n=== NCS at offset {} ===", offset);
            println!("  Header: comp={} decomp={}", header.compressed_size, header.decompressed_size);
            
            match decompress(&data[*offset..]) {
                Ok(decompressed) => {
                    println!("  Decompressed: {} bytes", decompressed.len());
                    
                    if let Some(content) = Content::parse(&decompressed) {
                        success += 1;
                        println!("  Type: {}", content.type_name());
                        println!("  Format: {}", content.format_code());
                        println!("  Strings: {}", content.strings.len());
                        for s in content.strings.iter().take(5) {
                            println!("    - {}", s);
                        }
                    } else {
                        failed_parse += 1;
                        println!("  FAILED to parse content");
                        // Show raw bytes
                        let hex: String = decompressed.iter().take(64)
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("  First 64 bytes: {}", hex);
                        // Show as string
                        let s: String = decompressed.iter().take(100)
                            .map(|&b| if b >= 32 && b < 127 { b as char } else { '.' })
                            .collect();
                        println!("  As string: {}", s);
                    }
                }
                Err(e) => println!("  Decompress error: {:?}", e),
            }
        }
        
        println!("\n\nSummary: {} parsed successfully, {} failed to parse", success, failed_parse);
    }
}

#[cfg(test)]
mod correlate_manifest {
    use crate::data::{decompress, scan};
    use crate::manifest::scan as scan_manifests;
    use crate::content::Content;
    use std::path::Path;

    #[test]
    fn find_missing_chunks() {
        let pak_path = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            return;
        }

        let data = std::fs::read(pak_path).expect("read pak");

        // Get manifest entries
        let manifests = scan_manifests(&data);
        let (_, manifest) = &manifests[0];

        // Sort entries by index
        let mut entries: Vec<_> = manifest.entries.iter().collect();
        entries.sort_by_key(|e| e.index);

        // Get NCS chunks sorted by offset
        let mut chunks = scan(&data);
        chunks.sort_by_key(|(offset, _)| *offset);

        println!("\nManifest: {} entries", entries.len());
        println!("Chunks: {} found", chunks.len());
        println!("Missing: {}", entries.len() as i32 - chunks.len() as i32);

        // Check index gaps - the index values should be sequential with stride 12
        println!("\nIndex gaps (missing slots):");
        let mut expected_index = entries[0].index;
        for entry in &entries {
            if entry.index != expected_index {
                let gap = (entry.index - expected_index) / 12;
                println!("  Gap before {}: {} missing slots (index jumped from {} to {})",
                    entry.filename, gap, expected_index, entry.index);
            }
            expected_index = entry.index + 12;
        }

        // The manifest has 170 entries but we found 164 chunks
        // This means 6 manifest entries don't have corresponding NCS data in this pak
        // OR there are 6 fewer chunks than manifest entries

        // Let's check: are there manifest entries beyond the chunks we found?
        println!("\nLast 10 manifest entries vs last chunks:");
        for i in (entries.len().saturating_sub(10))..entries.len() {
            let entry = &entries[i];
            let chunk_idx = if i < chunks.len() {
                Some(chunks[i].0)
            } else {
                None
            };
            println!("  {} (idx {}) -> chunk offset: {:?}",
                entry.filename, entry.index, chunk_idx);
        }
    }

    #[test]
    fn correlate_manifest_to_chunks() {
        let pak_path = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            return;
        }

        let data = std::fs::read(pak_path).expect("read pak");

        // Get manifest entries
        let manifests = scan_manifests(&data);
        println!("\nFound {} manifests", manifests.len());

        if manifests.is_empty() {
            println!("No manifests found!");
            return;
        }

        let (_, manifest) = &manifests[0];
        println!("Manifest has {} entries", manifest.entries.len());

        // Sort entries by index
        let mut entries: Vec<_> = manifest.entries.iter().collect();
        entries.sort_by_key(|e| e.index);

        // Get NCS chunks sorted by offset
        let mut chunks = scan(&data);
        chunks.sort_by_key(|(offset, _)| *offset);

        println!("Found {} NCS chunks", chunks.len());
        println!("\nCorrelating first 10:");
        println!("{:<6} {:<12} {:<40} {:<20}", "Idx", "Offset", "Manifest Filename", "Parsed Type");
        println!("{}", "-".repeat(80));

        for i in 0..10.min(entries.len().min(chunks.len())) {
            let entry = entries[i];
            let (offset, _header) = &chunks[i];

            // Try to parse content for comparison
            let parsed_type = match decompress(&data[*offset..]) {
                Ok(decompressed) => {
                    Content::parse(&decompressed)
                        .map(|c| c.type_name().to_string())
                        .unwrap_or_else(|| "(parse failed)".to_string())
                }
                Err(_) => "(decompress failed)".to_string(),
            };

            // Extract type from manifest filename: "Nexus-Data-{type}0.ncs"
            let manifest_type = entry.filename
                .strip_prefix("Nexus-Data-")
                .and_then(|s| s.strip_suffix("0.ncs"))
                .unwrap_or(&entry.filename);

            println!("{:<6} {:<12} {:<40} {:<20}",
                entry.index, offset, &entry.filename, parsed_type);

            // Check if they match (case-insensitive)
            if parsed_type.to_lowercase() != manifest_type.to_lowercase()
                && parsed_type != "(parse failed)" {
                println!("  ^ MISMATCH: manifest='{}' vs parsed='{}'", manifest_type, parsed_type);
            }
        }
    }
}

#[cfg(test)]
mod scan_all_paks {
    use crate::data::scan;
    use crate::manifest::scan as scan_manifests;
    use std::path::Path;

    #[test]
    fn scan_all_pak_ncs() {
        let paks_dir = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks");

        if !paks_dir.exists() {
            return;
        }

        println!("\n{:<45} {:>8} {:>10}", "Pak File", "Chunks", "Manifest");
        println!("{}", "-".repeat(65));

        let mut total_chunks = 0;
        let mut total_manifest_entries = 0;

        for entry in std::fs::read_dir(paks_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "pak") {
                let name = path.file_name().unwrap().to_string_lossy();

                let data = match std::fs::read(&path) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let chunks = scan(&data);
                let manifests = scan_manifests(&data);

                let manifest_count: usize = manifests.iter().map(|(_, m)| m.entries.len()).sum();

                if chunks.len() > 0 || manifest_count > 0 {
                    println!("{:<45} {:>8} {:>10}",
                        name, chunks.len(), manifest_count);
                    total_chunks += chunks.len();
                    total_manifest_entries += manifest_count;
                }
            }
        }

        println!("{}", "-".repeat(65));
        println!("{:<45} {:>8} {:>10}", "TOTAL", total_chunks, total_manifest_entries);
    }
}

#[cfg(test)]
mod test_extraction {
    use crate::extract::extract_from_pak;
    use std::path::Path;

    #[test]
    fn test_real_pak_extraction() {
        let pak_path = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            return;
        }

        let data = std::fs::read(pak_path).expect("read pak");
        let result = extract_from_pak(&data);

        println!("\n=== Extraction Results ===");
        println!("Files extracted: {}", result.files.len());
        println!("Missing chunks: {}", result.missing_chunks.len());
        println!("Orphan chunks: {}", result.orphan_chunks.len());

        println!("\nFirst 10 files:");
        for file in result.files.iter().take(10) {
            println!("  {} (type: {}, offset: {})",
                file.filename, file.type_name, file.offset);
        }

        println!("\nMissing chunks (manifest entries without data):");
        for entry in &result.missing_chunks {
            println!("  {} (index: {})", entry.filename, entry.index);
        }

        // Validate we got the expected count
        assert_eq!(result.files.len(), 164, "Expected 164 files");
        assert_eq!(result.missing_chunks.len(), 6, "Expected 6 missing");
        assert_eq!(result.orphan_chunks.len(), 0, "Expected 0 orphans");

        // Try decompressing a few files
        println!("\nDecompression test:");
        for file in result.files.iter().take(5) {
            match file.decompress(&data) {
                Ok(decompressed) => {
                    println!("  {} -> {} bytes", file.type_name, decompressed.len());
                }
                Err(e) => {
                    println!("  {} -> ERROR: {:?}", file.type_name, e);
                }
            }
        }
    }
}

#[cfg(test)]
mod investigate_missing {
    use crate::manifest::scan as scan_manifests;
    use std::path::Path;

    #[test]
    fn check_missing_entries() {
        let pak_path = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            return;
        }

        let data = std::fs::read(pak_path).expect("read pak");
        let manifests = scan_manifests(&data);
        let (_, manifest) = &manifests[0];

        // Get the 6 missing entries (last ones by index)
        let mut entries: Vec<_> = manifest.entries.iter().collect();
        entries.sort_by_key(|e| e.index);

        println!("\nLast 10 manifest entries:");
        for entry in entries.iter().rev().take(10).rev() {
            println!("  {} (index: {})", entry.filename, entry.index);
        }

        // Check the index gap pattern
        println!("\nIndex analysis for last entries:");
        let last_10: Vec<_> = entries.iter().rev().take(10).collect();
        for i in 0..last_10.len()-1 {
            let curr = last_10[i];
            let next = last_10[i+1];
            let gap = curr.index as i32 - next.index as i32;
            println!("  {} -> {}: gap = {}", curr.index, next.index, gap);
        }

        // Search for these specific strings in the pak to see if data exists
        println!("\nSearching for missing entry names in pak:");
        let missing = ["wwise_auxilary_busses", "wwise_soundbanks", "wwise_states", 
                       "wwise_switches", "wwise_triggers", "xp_progression"];
        
        for name in &missing {
            let pattern = name.as_bytes();
            let count = data.windows(pattern.len())
                .filter(|w| *w == pattern)
                .count();
            println!("  '{}': found {} times", name, count);
        }

        // Check what's at the expected offset region after the last known chunk
        // Last known chunk is at offset ~10602549 (wise_game_parameters)
        println!("\nData after last known NCS chunk (offset ~10602549):");
        let start = 10602549 + 1000; // A bit after the last chunk
        let preview: String = data[start..start+200].iter()
            .map(|&b| if b >= 32 && b < 127 { b as char } else { '.' })
            .collect();
        println!("  {}", preview);
    }
}

#[cfg(test)]
mod investigate_missing2 {
    use crate::data::scan;
    use std::path::Path;

    #[test]
    fn scan_after_last_chunk() {
        let pak_path = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            return;
        }

        let data = std::fs::read(pak_path).expect("read pak");
        
        // Get all chunks
        let mut chunks = scan(&data);
        chunks.sort_by_key(|(offset, _)| *offset);
        
        println!("\nLast 5 NCS chunks found:");
        for (offset, header) in chunks.iter().rev().take(5).rev() {
            println!("  offset {}: comp={} decomp={}", 
                offset, header.compressed_size, header.decompressed_size);
        }
        
        let last_offset = chunks.last().map(|(o, _)| *o).unwrap_or(0);
        let last_end = chunks.last().map(|(o, h)| o + h.total_size()).unwrap_or(0);
        
        println!("\nLast chunk ends at: {}", last_end);
        
        // Search for ANY NCS magic after the last chunk
        println!("\nSearching for NCS magic after last chunk...");
        let search_start = last_end;
        let ncs_magic = [0x4e, 0x43, 0x53]; // "NCS"
        
        let mut found = 0;
        for i in search_start..data.len().saturating_sub(3) {
            if data[i..i+3] == ncs_magic {
                found += 1;
                if found <= 10 {
                    let version = if i > 0 { data[i-1] } else { 0 };
                    println!("  Found at offset {} (version byte: 0x{:02x})", i-1, version);
                }
            }
        }
        println!("Total NCS magic found after last chunk: {}", found);
        
        // Check the manifest location
        println!("\nManifest is at offset 384699320");
        println!("Last NCS chunk at: {}", last_offset);
        println!("Gap between last chunk and manifest: {} bytes", 384699320 - last_end);
    }
}

#[cfg(test)]
mod full_mapping {
    use crate::extract::extract_from_pak;
    use std::path::Path;

    #[test]
    fn show_full_mapping() {
        let paks_dir = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks");

        if !paks_dir.exists() {
            return;
        }

        let mut total_files = 0;
        let mut total_missing = 0;
        let mut total_orphans = 0;

        println!("\n{:<45} {:>6} {:>6} {:>6}", "Pak File", "Files", "Miss", "Orph");
        println!("{}", "=".repeat(70));

        let mut paks: Vec<_> = std::fs::read_dir(paks_dir).unwrap()
            .flatten()
            .filter(|e| e.path().extension().map_or(false, |x| x == "pak"))
            .collect();
        paks.sort_by_key(|e| e.path());

        for entry in paks {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();

            let data = match std::fs::read(&path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let result = extract_from_pak(&data);

            if result.files.len() > 0 || result.missing_chunks.len() > 0 || result.orphan_chunks.len() > 0 {
                println!("{:<45} {:>6} {:>6} {:>6}",
                    name,
                    result.files.len(),
                    result.missing_chunks.len(),
                    result.orphan_chunks.len());

                total_files += result.files.len();
                total_missing += result.missing_chunks.len();
                total_orphans += result.orphan_chunks.len();
            }
        }

        println!("{}", "=".repeat(70));
        println!("{:<45} {:>6} {:>6} {:>6}", "TOTAL", total_files, total_missing, total_orphans);

        println!("\n\nSample mappings from pakchunk0:");
        let pak0 = paks_dir.join("pakchunk0-Windows_0_P.pak");
        let data = std::fs::read(&pak0).unwrap();
        let result = extract_from_pak(&data);

        println!("\n{:<5} {:<12} {:<40}", "Idx", "Offset", "Filename");
        println!("{}", "-".repeat(60));
        for file in result.files.iter().take(15) {
            println!("{:<5} {:<12} {:<40}", file.index, file.offset, file.filename);
        }
        println!("... ({} more)", result.files.len() - 15);
    }
}

#[cfg(test)]
mod generate_csv {
    use crate::extract::extract_from_pak;
    use std::path::Path;
    use std::io::Write;

    #[test]
    fn generate_mapping_csv() {
        let paks_dir = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks");

        if !paks_dir.exists() {
            return;
        }

        let mut csv = String::new();
        csv.push_str("pak_file,index,offset,compressed_size,decompressed_size,filename,type_name\n");

        let mut paks: Vec<_> = std::fs::read_dir(paks_dir).unwrap()
            .flatten()
            .filter(|e| e.path().extension().map_or(false, |x| x == "pak"))
            .collect();
        paks.sort_by_key(|e| e.path());

        for entry in paks {
            let path = entry.path();
            let pak_name = path.file_name().unwrap().to_string_lossy();

            let data = match std::fs::read(&path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let result = extract_from_pak(&data);

            for file in &result.files {
                csv.push_str(&format!("{},{},{},{},{},{},{}\n",
                    pak_name,
                    file.index,
                    file.offset,
                    file.header.compressed_size,
                    file.header.decompressed_size,
                    file.filename,
                    file.type_name));
            }
        }

        let out_path = "/home/polar/Documents/Projects/.bl4.info/ncs-mapping.csv";
        std::fs::write(out_path, &csv).expect("write csv");
        println!("\nWrote {} bytes to {}", csv.len(), out_path);
        
        // Show first 20 lines
        println!("\nFirst 20 lines:");
        for line in csv.lines().take(20) {
            println!("{}", line);
        }
        println!("...");
    }
}

#[cfg(test)]
mod test_parser_real {
    use crate::parser::parse_document;
    use std::path::Path;

    #[test]
    fn test_parser_on_files() {
        let test_dir = Path::new("/tmp/ncs_test");

        if !test_dir.exists() {
            println!("Test directory not found, skipping");
            return;
        }

        let mut success = 0;
        let mut failed = 0;
        let mut total_records = 0;

        for entry in std::fs::read_dir(test_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "bin") {
                let name = path.file_name().unwrap().to_string_lossy();
                let data = std::fs::read(&path).unwrap();

                match parse_document(&data) {
                    Some(doc) => {
                        success += 1;
                        total_records += doc.records.len();
                        if success <= 10 {
                            println!(
                                "{}: type={}, format={}, records={}",
                                name, doc.type_name, doc.format_code, doc.records.len()
                            );
                            if !doc.records.is_empty() {
                                println!("  First: {}", doc.records[0].name);
                            }
                        }
                    }
                    None => {
                        failed += 1;
                        if failed <= 10 {
                            println!("FAILED: {}", name);
                        }
                    }
                }
            }
        }

        println!("\n=== Summary ===");
        println!("Success: {}", success);
        println!("Failed: {}", failed);
        println!("Total records: {}", total_records);
        println!(
            "Success rate: {:.1}%",
            100.0 * success as f64 / (success + failed) as f64
        );
    }

    #[test]
    fn test_parser_json_output() {
        let test_file = Path::new("/tmp/ncs_test/achievement0.bin");

        if !test_file.exists() {
            println!("Test file not found, skipping");
            return;
        }

        let data = std::fs::read(test_file).unwrap();
        let doc = parse_document(&data).expect("Should parse achievement0");

        println!("Type: {}", doc.type_name);
        println!("Format: {}", doc.format_code);
        println!("Records: {}", doc.records.len());

        let json = serde_json::to_string_pretty(&doc).unwrap();
        println!("\nJSON Output:\n{}", json);
    }
}

#[cfg(test)]
mod investigate_inner_format {
    use oozextract::Extractor;
    use std::path::Path;

    #[test]
    fn try_header_offsets() {
        let pak_path = Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            return;
        }

        let data = std::fs::read(pak_path).unwrap();

        // attribute0 at offset 88117, compressed_size=66054, decompressed=325292
        let ncs_offset = 88117;
        let inner_start = ncs_offset + 16; // Skip 16-byte NCS header
        let compressed_size = 66054;
        let decompressed_size = 325292;

        println!("\n=== Inner header analysis ===");
        println!("NCS offset: {}", ncs_offset);
        println!("Inner start: {}", inner_start);
        println!("Compressed: {}, Decompressed: {}", compressed_size, decompressed_size);

        let inner = &data[inner_start..inner_start + 0x60.min(compressed_size)];
        println!("\nFirst 0x60 bytes of inner data:");
        for (i, chunk) in inner.chunks(16).enumerate() {
            print!("{:04x}: ", i * 16);
            for b in chunk {
                print!("{:02x} ", b);
            }
            println!();
        }

        let mut extractor = Extractor::new();

        println!("\n=== Trying different header skip values ===");
        for skip in [0, 4, 8, 16, 32, 48, 64, 72, 80, 96, 128] {
            if skip >= compressed_size {
                continue;
            }

            let end = inner_start + compressed_size;
            let oodle_data = &data[inner_start + skip..end];
            let mut output = vec![0u8; decompressed_size];

            match extractor.read_from_slice(oodle_data, &mut output) {
                Ok(actual) => {
                    if actual > 10000 {  // Significant output
                        println!("Skip {}: SUCCESS {} bytes", skip, actual);
                        println!("  First 32: {:02x?}", &output[..32.min(actual)]);
                    }
                }
                Err(e) => {
                    if skip < 100 {
                        println!("Skip {}: {:?}", skip, e);
                    }
                }
            }
        }

        // Also try: what if we need to decompress block by block?
        // Check bytes at offset 12-15 for potential block_count
        let block_count = u32::from_be_bytes([inner[12], inner[13], inner[14], inner[15]]);
        println!("\n=== Block analysis ===");
        println!("Potential block_count (bytes 12-15 BE): {}", block_count);

        // If block_count = 2, try decompressing as multiple blocks
        if block_count > 0 && block_count < 100 {
            println!("Trying multi-block decompression with {} blocks...", block_count);
        }
    }
}
