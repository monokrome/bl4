//! Part definition extraction from memory
//!
//! Extracts InventoryPartDef objects and their SerialIndex data from game memory.
//! This module stores raw binary data without making assumptions about field meanings.

use anyhow::{bail, Result};
use byteorder::{ByteOrder, LE};
use serde::{Deserialize, Serialize};

use super::constants::{MAX_VALID_POINTER, MIN_VALID_POINTER};
use super::fname::FNamePool;
use super::pattern::scan_pattern_fast;
use super::source::MemorySource;

/// Raw SerialIndex data extracted from memory
///
/// We read 16 bytes starting at UObject+0x28 and store them with multiple interpretations.
/// This avoids making assumptions about field layouts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPartData {
    /// Part name from FName (e.g., "DAD_PS.part_barrel_01")
    pub name: String,

    /// Memory address of the UObject
    #[serde(serialize_with = "serialize_hex", deserialize_with = "deserialize_hex")]
    pub address: usize,

    /// Raw bytes at UObject+0x28 (hex string for readability)
    pub raw_hex: String,

    /// Bytes interpreted as 8 u8 values
    pub as_u8: [u8; 16],

    /// Bytes interpreted as 8 i16 values (little-endian)
    pub as_i16: [i16; 8],

    /// Bytes interpreted as 4 i32 values (little-endian)
    pub as_i32: [i32; 4],

    /// Bytes interpreted as 2 i64 values (little-endian)
    pub as_i64: [i64; 2],
}

fn serialize_hex<S>(addr: &usize, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&format!("{:#x}", addr))
}

fn deserialize_hex<'de, D>(d: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    usize::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
}

/// Extraction output with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartsExtraction {
    /// Extraction timestamp
    pub extracted_at: String,

    /// Source file (dump path or "live")
    pub source: String,

    /// Offset from UObject base where SerialIndex was read
    pub serial_index_offset: usize,

    /// Number of bytes read at that offset
    pub bytes_read: usize,

    /// Extracted parts with raw data
    pub parts: Vec<RawPartData>,
}

/// Search FNamePool for all names containing ".part_"
fn find_part_fnames(
    source: &dyn MemorySource,
    pool: &FNamePool,
) -> Result<std::collections::HashMap<u32, String>> {
    let mut results = std::collections::HashMap::new();

    let max_blocks = pool.blocks.len().min(400);
    for (block_idx, &block_ptr) in pool.blocks.iter().take(max_blocks).enumerate() {
        if block_ptr < MIN_VALID_POINTER || block_ptr > MAX_VALID_POINTER {
            continue;
        }

        // Read block entries (each entry is variable length)
        let block_data = match source.read_bytes(block_ptr, 0x10000) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let mut offset = 0;
        while offset + 4 < block_data.len() {
            let header = LE::read_u16(&block_data[offset..offset + 2]);
            let len = (header >> 6) as usize;

            if len == 0 || len > 1024 || offset + 2 + len > block_data.len() {
                offset += 2;
                continue;
            }

            if let Ok(name) = std::str::from_utf8(&block_data[offset + 2..offset + 2 + len]) {
                if name.contains(".part_") {
                    let fname_idx = (block_idx as u32) << 16 | (offset as u32 / 2);
                    results.insert(fname_idx, name.to_string());
                }
            }

            offset += 2 + len;
            // Align to 2 bytes
            if offset % 2 != 0 {
                offset += 1;
            }
        }
    }

    Ok(results)
}

/// Extract part definitions with raw binary data (no assumptions)
pub fn extract_parts_raw(source: &dyn MemorySource) -> Result<PartsExtraction> {
    eprintln!("Extracting parts with raw binary data...");

    // Discover FNamePool
    let pool = FNamePool::discover(source)?;

    // Find all part FNames
    eprintln!("Step 1: Finding part FNames...");
    let part_fnames = find_part_fnames(source, &pool)?;
    eprintln!("Found {} FNames containing '.part_'", part_fnames.len());

    if part_fnames.is_empty() {
        bail!("No part FNames found");
    }

    // Scan for part registration entries (0xFFFFFFFF markers)
    eprintln!("Step 2: Scanning for part objects...");

    let marker_pattern = [0xFF, 0xFF, 0xFF, 0xFF];
    let mask = vec![1u8; 4];

    let mut parts = Vec::new();
    let mut seen_addresses = std::collections::HashSet::new();
    let mut processed_regions = 0;

    const SERIAL_INDEX_OFFSET: usize = 0x28;
    const BYTES_TO_READ: usize = 16;

    for region in source.regions() {
        if region.size() < 1024 || region.size() > 500 * 1024 * 1024 {
            continue;
        }
        if !region.is_readable() {
            continue;
        }

        processed_regions += 1;
        if processed_regions % 500 == 0 {
            eprintln!("  Processed {} regions, found {} parts...", processed_regions, parts.len());
        }

        let region_data = match source.read_bytes(region.start, region.size()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let marker_offsets = scan_pattern_fast(&region_data, &marker_pattern, &mask);

        for &marker_offset in &marker_offsets {
            // Entry: FName(4) + padding(4) + pointer(8) + marker(4)
            if marker_offset < 16 {
                continue;
            }
            let entry_offset = marker_offset - 16;
            if entry_offset + 24 > region_data.len() {
                continue;
            }

            let entry_data = &region_data[entry_offset..entry_offset + 24];

            let fname_idx = LE::read_u32(&entry_data[0..4]);
            let padding = LE::read_u32(&entry_data[4..8]);
            let pointer = LE::read_u64(&entry_data[8..16]) as usize;
            let marker = LE::read_u32(&entry_data[16..20]);

            if marker != 0xFFFFFFFF || padding != 0 {
                continue;
            }
            if pointer < MIN_VALID_POINTER || pointer > MAX_VALID_POINTER {
                continue;
            }

            // Check if this FName is a known part
            let name = match part_fnames.get(&fname_idx) {
                Some(n) => n.clone(),
                None => continue,
            };

            // Skip duplicates
            if seen_addresses.contains(&pointer) {
                continue;
            }
            seen_addresses.insert(pointer);

            // Read raw bytes at SerialIndex offset
            let raw_bytes = match source.read_bytes(pointer + SERIAL_INDEX_OFFSET, BYTES_TO_READ) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Store with multiple interpretations
            let raw_hex = raw_bytes.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");

            let mut as_u8 = [0u8; 16];
            as_u8.copy_from_slice(&raw_bytes);

            let as_i16 = [
                LE::read_i16(&raw_bytes[0..2]),
                LE::read_i16(&raw_bytes[2..4]),
                LE::read_i16(&raw_bytes[4..6]),
                LE::read_i16(&raw_bytes[6..8]),
                LE::read_i16(&raw_bytes[8..10]),
                LE::read_i16(&raw_bytes[10..12]),
                LE::read_i16(&raw_bytes[12..14]),
                LE::read_i16(&raw_bytes[14..16]),
            ];

            let as_i32 = [
                LE::read_i32(&raw_bytes[0..4]),
                LE::read_i32(&raw_bytes[4..8]),
                LE::read_i32(&raw_bytes[8..12]),
                LE::read_i32(&raw_bytes[12..16]),
            ];

            let as_i64 = [
                LE::read_i64(&raw_bytes[0..8]),
                LE::read_i64(&raw_bytes[8..16]),
            ];

            parts.push(RawPartData {
                name,
                address: pointer,
                raw_hex,
                as_u8,
                as_i16,
                as_i32,
                as_i64,
            });
        }
    }

    eprintln!("Extraction complete: {} parts from {} regions", parts.len(), processed_regions);

    // Sort by name for consistency
    parts.sort_by(|a, b| a.name.cmp(&b.name));

    // Get timestamp using std
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Ok(PartsExtraction {
        extracted_at: format!("{}", now),
        source: "minidump".to_string(),
        serial_index_offset: SERIAL_INDEX_OFFSET,
        bytes_read: BYTES_TO_READ,
        parts,
    })
}
