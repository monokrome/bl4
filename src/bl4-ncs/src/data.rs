//! NCS data format parsing (`[version]NCS`)
//!
//! NCS files are Oodle-compressed configuration stores.

use memchr::memmem;

use crate::{Error, Result, NCS_MAGIC, OODLE_MAGIC};

/// Header size in bytes
pub const HEADER_SIZE: usize = 16;

/// Inner header minimum size (when compression_type == 0)
pub const INNER_HEADER_MIN: usize = 0x40;

// Inner header field offsets
const INNER_COMPRESSION_TYPE: usize = 0x18;
const INNER_BLOCK_COUNT: usize = 0x1c;
const INNER_RAW_DATA_START: usize = 0x50;

/// NCS file header (16 bytes)
#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// Version byte (typically 0x01)
    pub version: u8,
    /// Compression flag (0 = uncompressed, non-zero = Oodle compressed)
    pub compression_flag: u32,
    /// Size after decompression
    pub decompressed_size: u32,
    /// Size of compressed data
    pub compressed_size: u32,
}

impl Header {
    /// Parse NCS header directly from slice (zero-copy)
    #[inline]
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(Error::DataTooShort {
                needed: HEADER_SIZE,
                actual: data.len(),
            });
        }

        if data[1..4] != NCS_MAGIC {
            return Err(Error::InvalidNcsMagic(data[1], data[2], data[3]));
        }

        Ok(Self {
            version: data[0],
            compression_flag: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            decompressed_size: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            compressed_size: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        })
    }

    #[inline]
    pub fn is_compressed(&self) -> bool {
        self.compression_flag != 0
    }

    #[inline]
    pub fn total_size(&self) -> usize {
        HEADER_SIZE + self.compressed_size as usize
    }
}

/// Decompress an NCS chunk
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let header = Header::from_bytes(data)?;

    let payload = &data[HEADER_SIZE..];
    if payload.len() < header.compressed_size as usize {
        return Err(Error::DataTooShort {
            needed: header.compressed_size as usize,
            actual: payload.len(),
        });
    }

    let compressed = &payload[..header.compressed_size as usize];

    if !header.is_compressed() {
        return Ok(compressed.to_vec());
    }

    decompress_inner(compressed, header.decompressed_size as usize)
}

/// Decompress the inner Oodle-compressed payload
fn decompress_inner(compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
    if compressed.len() < INNER_HEADER_MIN {
        return Err(Error::DataTooShort {
            needed: INNER_HEADER_MIN,
            actual: compressed.len(),
        });
    }

    let inner_magic = u32::from_be_bytes([compressed[0], compressed[1], compressed[2], compressed[3]]);
    if inner_magic != OODLE_MAGIC {
        return Err(Error::InvalidInnerMagic(inner_magic));
    }

    let compression_type = compressed[INNER_COMPRESSION_TYPE];
    let block_count = u32::from_be_bytes([
        compressed[INNER_BLOCK_COUNT],
        compressed[INNER_BLOCK_COUNT + 1],
        compressed[INNER_BLOCK_COUNT + 2],
        compressed[INNER_BLOCK_COUNT + 3],
    ]);

    // Raw data when compression_type == 0
    if compression_type == 0 {
        return Ok(compressed[INNER_RAW_DATA_START..].to_vec());
    }

    // Oodle compressed: header size includes block offset table
    let inner_header_size = INNER_HEADER_MIN + (block_count as usize * 4);
    if compressed.len() < inner_header_size {
        return Err(Error::DataTooShort {
            needed: inner_header_size,
            actual: compressed.len(),
        });
    }

    let oodle_data = &compressed[inner_header_size..];
    let mut decompressed = vec![0u8; decompressed_size];
    let mut extractor = oozextract::Extractor::new();

    let actual = extractor
        .read_from_slice(oodle_data, &mut decompressed)
        .map_err(|e| Error::Oodle(format!("{:?}", e)))?;

    if actual != decompressed_size {
        return Err(Error::DecompressionSize {
            expected: decompressed_size,
            actual,
        });
    }

    Ok(decompressed)
}

/// Scan for NCS data chunks in binary data
pub fn scan(data: &[u8]) -> Vec<(usize, Header)> {
    let finder = memmem::Finder::new(&NCS_MAGIC);
    let mut results = Vec::new();

    for offset in finder.find_iter(data) {
        // NCS magic is at bytes 1-3, version byte is at offset-1
        if offset == 0 {
            continue;
        }

        let start = offset - 1;

        // Skip if this is actually a manifest (_NCS/)
        if start > 0 && data[start - 1] == b'_' {
            continue;
        }

        if let Ok(header) = Header::from_bytes(&data[start..]) {
            if start + header.total_size() <= data.len() {
                results.push((start, header));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(HEADER_SIZE, 16);
    }

    #[test]
    fn test_header_parse_too_short() {
        let data = [0u8; 8];
        assert!(Header::from_bytes(&data).is_err());
    }
}
