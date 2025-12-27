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
