//! Legacy gBx format support (deprecated)
//!
//! The gBx format detection was based on false positives from compressed data.
//! This module is kept for backwards compatibility but should not be used for new code.
//! Use the NCS format (`is_ncs`, `decompress_ncs`) instead.

#![deprecated(since = "0.5.0", note = "gBx format was based on false positives; use NCS format instead")]

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::{Error, Result};

/// Magic bytes for gBx format
pub const GBX_MAGIC: [u8; 3] = [0x67, 0x42, 0x78]; // "gBx"

/// Known gBx variant bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Variant {
    V9 = 0x39,
    V6 = 0x36,
    Vr = 0x72,
    VEF = 0xEF,
    VE0 = 0xE0,
}

impl TryFrom<u8> for Variant {
    type Error = Error;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x39 => Ok(Self::V9),
            0x36 => Ok(Self::V6),
            0x72 => Ok(Self::Vr),
            0xEF => Ok(Self::VEF),
            0xE0 => Ok(Self::VE0),
            _ => Err(Error::UnknownVariant(value)),
        }
    }
}

/// gBx file header
#[derive(Debug, Clone)]
pub struct Header {
    pub variant: Variant,
    pub compressed_size: u32,
    pub decompressed_size: u32,
    pub data_offset: u64,
}

impl Header {
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 3];
        reader.read_exact(&mut magic)?;

        if magic != GBX_MAGIC {
            return Err(Error::InvalidMagic(magic[0], magic[1], magic[2]));
        }

        let variant = Variant::try_from(reader.read_u8()?)?;
        let decompressed_size = reader.read_u32::<LittleEndian>()?;
        let compressed_size = reader.read_u32::<LittleEndian>()?;
        let data_offset = reader.stream_position()?;

        Ok(Self {
            variant,
            compressed_size,
            decompressed_size,
            data_offset,
        })
    }
}

/// Check if data starts with gBx magic
pub fn is_gbx(data: &[u8]) -> bool {
    data.len() >= 4 && data[0..3] == GBX_MAGIC
}

/// Get the variant without full parsing
pub fn get_variant(data: &[u8]) -> Option<Variant> {
    if data.len() >= 4 && data[0..3] == GBX_MAGIC {
        Variant::try_from(data[3]).ok()
    } else {
        None
    }
}

/// Decompress gBx data
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(data);
    let header = Header::parse(&mut cursor)?;

    cursor.seek(SeekFrom::Start(header.data_offset))?;

    let mut compressed = vec![0u8; header.compressed_size as usize];
    cursor.read_exact(&mut compressed)?;

    let mut decompressed = vec![0u8; header.decompressed_size as usize];
    let mut extractor = oozextract::Extractor::new();
    let actual_size = extractor
        .read_from_slice(&compressed, &mut decompressed)
        .map_err(|e| Error::Oodle(format!("Decompression failed: {:?}", e)))?;

    if actual_size != header.decompressed_size as usize {
        return Err(Error::DecompressionSize {
            expected: header.decompressed_size as usize,
            actual: actual_size,
        });
    }

    Ok(decompressed)
}

/// Information about a gBx chunk found during scanning
#[derive(Debug, Clone)]
pub struct Chunk {
    pub offset: u64,
    pub header: Header,
    pub total_size: u64,
}

/// Scan a reader for gBx chunks
pub fn scan<R: Read + Seek>(reader: &mut R) -> Result<Vec<Chunk>> {
    use memchr::memmem;

    let start = reader.stream_position()?;
    reader.seek(SeekFrom::End(0))?;
    let file_size = reader.stream_position()?;
    reader.seek(SeekFrom::Start(start))?;

    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let finder = memmem::Finder::new(&GBX_MAGIC);
    let mut chunks = Vec::new();

    for offset in finder.find_iter(&data) {
        if offset + 12 > data.len() {
            continue;
        }

        let mut cursor = Cursor::new(&data[offset..]);
        if let Ok(header) = Header::parse(&mut cursor) {
            let total_size = header.data_offset + header.compressed_size as u64;
            if offset as u64 + total_size <= file_size {
                chunks.push(Chunk {
                    offset: offset as u64,
                    header,
                    total_size,
                });
            }
        }
    }

    Ok(chunks)
}

/// Result of scanning for gBx magic (includes invalid matches)
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub offset: u64,
    pub variant: Option<Variant>,
    pub decompressed_size: u32,
    pub compressed_size: u32,
    pub valid: bool,
    pub invalid_reason: Option<String>,
}

/// Scan a reader for all gBx magic occurrences (including invalid ones)
pub fn scan_all<R: Read + Seek>(reader: &mut R) -> Result<Vec<ScanResult>> {
    use memchr::memmem;

    let start = reader.stream_position()?;
    reader.seek(SeekFrom::End(0))?;
    let file_size = reader.stream_position()?;
    reader.seek(SeekFrom::Start(start))?;

    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let finder = memmem::Finder::new(&GBX_MAGIC);
    let mut results = Vec::new();

    for offset in finder.find_iter(&data) {
        if offset + 12 > data.len() {
            continue;
        }

        let mut cursor = Cursor::new(&data[offset..]);
        cursor.set_position(3);

        let variant_byte = cursor.read_u8().unwrap_or(0);
        let variant = Variant::try_from(variant_byte).ok();
        let decompressed_size = cursor.read_u32::<LittleEndian>().unwrap_or(0);
        let compressed_size = cursor.read_u32::<LittleEndian>().unwrap_or(0);

        let total_size = 12u64 + compressed_size as u64;
        let fits = offset as u64 + total_size <= file_size;

        let (valid, invalid_reason) = if variant.is_none() {
            (false, Some(format!("unknown variant 0x{:02x}", variant_byte)))
        } else if !fits {
            (false, Some(format!("size {} exceeds file", compressed_size)))
        } else {
            (true, None)
        };

        results.push(ScanResult {
            offset: offset as u64,
            variant,
            decompressed_size,
            compressed_size,
            valid,
            invalid_reason,
        });
    }

    Ok(results)
}

/// Extract a gBx chunk from a reader
pub fn extract_chunk<R: Read + Seek>(reader: &mut R, chunk: &Chunk) -> Result<Vec<u8>> {
    reader.seek(SeekFrom::Start(chunk.offset))?;
    let mut data = vec![0u8; chunk.total_size as usize];
    reader.read_exact(&mut data)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a valid gBx header with given parameters
    fn make_gbx_header(variant: u8, decompressed: u32, compressed: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&GBX_MAGIC);
        data.push(variant);
        data.extend_from_slice(&decompressed.to_le_bytes());
        data.extend_from_slice(&compressed.to_le_bytes());
        data
    }

    #[test]
    fn test_gbx_magic() {
        assert_eq!(GBX_MAGIC, [0x67, 0x42, 0x78]);
        assert_eq!(&GBX_MAGIC, b"gBx");
    }

    #[test]
    fn test_variant_values() {
        assert_eq!(Variant::V9 as u8, 0x39);
        assert_eq!(Variant::V6 as u8, 0x36);
        assert_eq!(Variant::Vr as u8, 0x72);
        assert_eq!(Variant::VEF as u8, 0xEF);
        assert_eq!(Variant::VE0 as u8, 0xE0);
    }

    #[test]
    fn test_variant_try_from_valid() {
        assert_eq!(Variant::try_from(0x39).unwrap(), Variant::V9);
        assert_eq!(Variant::try_from(0x36).unwrap(), Variant::V6);
        assert_eq!(Variant::try_from(0x72).unwrap(), Variant::Vr);
        assert_eq!(Variant::try_from(0xEF).unwrap(), Variant::VEF);
        assert_eq!(Variant::try_from(0xE0).unwrap(), Variant::VE0);
    }

    #[test]
    fn test_variant_try_from_invalid() {
        let result = Variant::try_from(0x00);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::UnknownVariant(0x00)));

        let result = Variant::try_from(0xFF);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::UnknownVariant(0xFF)));
    }

    #[test]
    fn test_variant_debug() {
        let debug = format!("{:?}", Variant::V9);
        assert!(debug.contains("V9"));
    }

    #[test]
    fn test_variant_clone_eq() {
        let v1 = Variant::V9;
        let v2 = v1;
        assert_eq!(v1, v2);
        assert_ne!(Variant::V9, Variant::V6);
    }

    #[test]
    fn test_header_parse_valid() {
        let data = make_gbx_header(0x39, 1000, 500);
        let mut cursor = Cursor::new(&data);
        let header = Header::parse(&mut cursor).unwrap();

        assert_eq!(header.variant, Variant::V9);
        assert_eq!(header.decompressed_size, 1000);
        assert_eq!(header.compressed_size, 500);
        assert_eq!(header.data_offset, 12);
    }

    #[test]
    fn test_header_parse_all_variants() {
        for (byte, expected) in [
            (0x39, Variant::V9),
            (0x36, Variant::V6),
            (0x72, Variant::Vr),
            (0xEF, Variant::VEF),
            (0xE0, Variant::VE0),
        ] {
            let data = make_gbx_header(byte, 100, 50);
            let mut cursor = Cursor::new(&data);
            let header = Header::parse(&mut cursor).unwrap();
            assert_eq!(header.variant, expected);
        }
    }

    #[test]
    fn test_header_parse_invalid_magic() {
        let data = [0x00, 0x00, 0x00, 0x39, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(&data);
        let result = Header::parse(&mut cursor);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidMagic(0x00, 0x00, 0x00)));
    }

    #[test]
    fn test_header_parse_unknown_variant() {
        let mut data = Vec::new();
        data.extend_from_slice(&GBX_MAGIC);
        data.push(0x00);  // Invalid variant
        data.extend_from_slice(&[0u8; 8]);

        let mut cursor = Cursor::new(&data);
        let result = Header::parse(&mut cursor);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::UnknownVariant(0x00)));
    }

    #[test]
    fn test_header_parse_too_short() {
        let data = [0x67, 0x42, 0x78];  // Just magic, no variant or sizes
        let mut cursor = Cursor::new(&data);
        let result = Header::parse(&mut cursor);
        assert!(result.is_err());
    }

    #[test]
    fn test_header_debug() {
        let data = make_gbx_header(0x39, 100, 50);
        let mut cursor = Cursor::new(&data);
        let header = Header::parse(&mut cursor).unwrap();
        let debug = format!("{:?}", header);
        assert!(debug.contains("Header"));
        assert!(debug.contains("variant"));
    }

    #[test]
    fn test_is_gbx_valid() {
        let data = make_gbx_header(0x39, 100, 50);
        assert!(is_gbx(&data));
    }

    #[test]
    fn test_is_gbx_invalid_magic() {
        assert!(!is_gbx(&[0x00, 0x00, 0x00, 0x00]));
        assert!(!is_gbx(&[0x67, 0x00, 0x00, 0x00]));  // Wrong second byte
        assert!(!is_gbx(&[0x67, 0x42, 0x00, 0x00]));  // Wrong third byte
    }

    #[test]
    fn test_is_gbx_too_short() {
        assert!(!is_gbx(&[]));
        assert!(!is_gbx(&[0x67]));
        assert!(!is_gbx(&[0x67, 0x42]));
        assert!(!is_gbx(&[0x67, 0x42, 0x78]));  // Exactly magic, no variant
    }

    #[test]
    fn test_get_variant_valid() {
        let data = make_gbx_header(0x39, 100, 50);
        assert_eq!(get_variant(&data), Some(Variant::V9));

        let data = make_gbx_header(0x36, 100, 50);
        assert_eq!(get_variant(&data), Some(Variant::V6));
    }

    #[test]
    fn test_get_variant_unknown() {
        let mut data = Vec::new();
        data.extend_from_slice(&GBX_MAGIC);
        data.push(0x00);  // Unknown variant
        assert_eq!(get_variant(&data), None);
    }

    #[test]
    fn test_get_variant_invalid_magic() {
        assert_eq!(get_variant(&[0x00, 0x00, 0x00, 0x39]), None);
    }

    #[test]
    fn test_get_variant_too_short() {
        assert_eq!(get_variant(&[]), None);
        assert_eq!(get_variant(&[0x67, 0x42, 0x78]), None);  // No variant byte
    }

    #[test]
    fn test_chunk_debug() {
        let data = make_gbx_header(0x39, 100, 50);
        let mut cursor = Cursor::new(&data);
        let header = Header::parse(&mut cursor).unwrap();
        let chunk = Chunk {
            offset: 0,
            header,
            total_size: 62,
        };
        let debug = format!("{:?}", chunk);
        assert!(debug.contains("Chunk"));
        assert!(debug.contains("offset"));
    }

    #[test]
    fn test_scan_empty() {
        let data: &[u8] = &[];
        let mut cursor = Cursor::new(data);
        let results = scan(&mut cursor).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_no_gbx() {
        let data = b"Some random data without gBx magic";
        let mut cursor = Cursor::new(&data[..]);
        let results = scan(&mut cursor).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_single_chunk() {
        let mut data = make_gbx_header(0x39, 8, 8);
        data.extend_from_slice(&[0u8; 8]);  // Compressed payload

        let mut cursor = Cursor::new(&data[..]);
        let results = scan(&mut cursor).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].header.variant, Variant::V9);
        assert_eq!(results[0].total_size, 20);  // 12 header + 8 payload
    }

    #[test]
    fn test_scan_with_padding() {
        let mut data = vec![0xFFu8; 20];  // Padding
        let gbx_start = data.len();
        data.extend_from_slice(&make_gbx_header(0x36, 4, 4));
        data.extend_from_slice(&[0u8; 4]);  // Payload

        let mut cursor = Cursor::new(&data[..]);
        let results = scan(&mut cursor).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offset, gbx_start as u64);
    }

    #[test]
    fn test_scan_truncated_header() {
        // gBx magic but not enough bytes for full header
        let data = [0x67, 0x42, 0x78, 0x39, 0x00, 0x00];  // Only 6 bytes
        let mut cursor = Cursor::new(&data[..]);
        let results = scan(&mut cursor).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_size_exceeds_file() {
        // Header claims more compressed data than file contains
        let data = make_gbx_header(0x39, 100, 1000);  // Claims 1000 bytes compressed
        let mut cursor = Cursor::new(&data[..]);
        let results = scan(&mut cursor).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_all_empty() {
        let data: &[u8] = &[];
        let mut cursor = Cursor::new(data);
        let results = scan_all(&mut cursor).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_all_valid() {
        let mut data = make_gbx_header(0x39, 8, 8);
        data.extend_from_slice(&[0u8; 8]);

        let mut cursor = Cursor::new(&data[..]);
        let results = scan_all(&mut cursor).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].valid);
        assert!(results[0].invalid_reason.is_none());
        assert_eq!(results[0].variant, Some(Variant::V9));
    }

    #[test]
    fn test_scan_all_unknown_variant() {
        let mut data = Vec::new();
        data.extend_from_slice(&GBX_MAGIC);
        data.push(0x00);  // Unknown variant
        data.extend_from_slice(&0u32.to_le_bytes());  // decompressed
        data.extend_from_slice(&0u32.to_le_bytes());  // compressed

        let mut cursor = Cursor::new(&data[..]);
        let results = scan_all(&mut cursor).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].valid);
        assert!(results[0].variant.is_none());
        assert!(results[0].invalid_reason.as_ref().unwrap().contains("unknown variant"));
    }

    #[test]
    fn test_scan_all_size_exceeds() {
        let data = make_gbx_header(0x39, 100, 1000);  // Claims 1000 bytes
        let mut cursor = Cursor::new(&data[..]);
        let results = scan_all(&mut cursor).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].valid);
        assert!(results[0].invalid_reason.as_ref().unwrap().contains("exceeds file"));
    }

    #[test]
    fn test_scan_result_debug() {
        let result = ScanResult {
            offset: 0,
            variant: Some(Variant::V9),
            decompressed_size: 100,
            compressed_size: 50,
            valid: true,
            invalid_reason: None,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("ScanResult"));
        assert!(debug.contains("valid"));
    }

    #[test]
    fn test_extract_chunk() {
        let mut data = vec![0xAAu8; 10];  // Padding
        let gbx_start = data.len();
        let header_data = make_gbx_header(0x39, 4, 4);
        data.extend_from_slice(&header_data);
        data.extend_from_slice(&[0xBB, 0xCC, 0xDD, 0xEE]);  // Payload

        // Parse to get chunk info
        let mut cursor = Cursor::new(&data[..]);
        let chunks = scan(&mut cursor).unwrap();
        assert_eq!(chunks.len(), 1);

        // Extract the chunk
        cursor.seek(SeekFrom::Start(0)).unwrap();
        let extracted = extract_chunk(&mut cursor, &chunks[0]).unwrap();

        // Should be header + payload
        assert_eq!(extracted.len(), 16);  // 12 header + 4 payload
        assert_eq!(&extracted[0..3], &GBX_MAGIC);
        assert_eq!(&extracted[12..16], &[0xBB, 0xCC, 0xDD, 0xEE]);
    }

    #[test]
    fn test_decompress_invalid_magic() {
        let data = [0x00u8; 20];
        let result = decompress(&data);
        assert!(result.is_err());
    }
}
