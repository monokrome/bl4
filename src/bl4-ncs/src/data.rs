//! NCS data format parsing (`[version]NCS`)
//!
//! NCS files are Oodle-compressed configuration stores.
//!
//! # Inner Format
//!
//! The inner compressed payload has this structure:
//! - 0x00-0x03: Oodle magic (0xb7756362, big-endian)
//! - 0x0c-0x0f: Block count (big-endian)
//! - 0x40+: Block size table (block_count * 4 bytes, each big-endian)
//! - After table: Concatenated Oodle-compressed blocks
//!
//! Each block decompresses to up to 256KB (BLOCK_DECOMP_SIZE), except the last
//! block which may be smaller.

use memchr::memmem;

use crate::oodle::{OodleDecompressor, OozextractBackend};
use crate::{Error, Result, NCS_MAGIC, OODLE_MAGIC};

/// Header size in bytes
pub const HEADER_SIZE: usize = 16;

/// Inner header size (before block table)
pub const INNER_HEADER_MIN: usize = 0x40;

/// Maximum decompressed size per block (256KB)
const BLOCK_DECOMP_SIZE: usize = 0x40000;

// Inner header field offsets
const INNER_BLOCK_COUNT: usize = 0x0c;

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

/// Decompress an NCS chunk using the default backend (oozextract)
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let backend = OozextractBackend::new();
    decompress_with(data, &backend)
}

/// Decompress an NCS chunk using a specific Oodle backend
pub fn decompress_with(data: &[u8], decompressor: &dyn OodleDecompressor) -> Result<Vec<u8>> {
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

    decompress_inner(compressed, header.decompressed_size as usize, decompressor)
}

// Inner header format flags offset (bytes 0x08-0x0b)
const INNER_FORMAT_FLAGS: usize = 0x08;

/// Decompress the inner Oodle-compressed payload
///
/// The inner format has two variants based on bytes 0x08-0x0b:
/// - Multi-block (0x03030812): Block sizes at 0x40, data follows
/// - Single-block (0x00000000): Data directly at 0x40
fn decompress_inner(
    compressed: &[u8],
    decompressed_size: usize,
    decompressor: &dyn OodleDecompressor,
) -> Result<Vec<u8>> {
    if compressed.len() < INNER_HEADER_MIN {
        return Err(Error::DataTooShort {
            needed: INNER_HEADER_MIN,
            actual: compressed.len(),
        });
    }

    // Validate Oodle magic
    let inner_magic =
        u32::from_be_bytes([compressed[0], compressed[1], compressed[2], compressed[3]]);
    if inner_magic != OODLE_MAGIC {
        return Err(Error::InvalidInnerMagic(inner_magic));
    }

    // Check format flags at 0x08-0x0b to determine inner format
    let format_flags = u32::from_be_bytes([
        compressed[INNER_FORMAT_FLAGS],
        compressed[INNER_FORMAT_FLAGS + 1],
        compressed[INNER_FORMAT_FLAGS + 2],
        compressed[INNER_FORMAT_FLAGS + 3],
    ]);

    // Single-block format: format flags are 0, data starts at 0x40
    if format_flags == 0 {
        return decompress_single_block(compressed, decompressed_size, decompressor);
    }

    // Multi-block format: block count at 0x0c, table at 0x40
    decompress_multi_block(compressed, decompressed_size, decompressor)
}

/// Decompress single-block format (format flags = 0)
///
/// Two cases:
/// - If data after header equals decompressed_size: data is uncompressed (raw)
/// - Otherwise: data is Oodle-compressed single block
fn decompress_single_block(
    compressed: &[u8],
    decompressed_size: usize,
    decompressor: &dyn OodleDecompressor,
) -> Result<Vec<u8>> {
    let data_start = INNER_HEADER_MIN;
    let block_data = &compressed[data_start..];

    // If data size matches decompressed size, it's uncompressed (raw)
    if block_data.len() == decompressed_size {
        return Ok(block_data.to_vec());
    }

    // Otherwise, decompress as single Oodle block
    decompressor.decompress_block(block_data, decompressed_size)
}

/// Decompress multi-block format (format flags = 0x03030812)
///
/// Block count at 0x0c, block sizes at 0x40, data follows table.
fn decompress_multi_block(
    compressed: &[u8],
    decompressed_size: usize,
    decompressor: &dyn OodleDecompressor,
) -> Result<Vec<u8>> {
    // Block count is at offset 0x0c (big-endian)
    let block_count = u32::from_be_bytes([
        compressed[INNER_BLOCK_COUNT],
        compressed[INNER_BLOCK_COUNT + 1],
        compressed[INNER_BLOCK_COUNT + 2],
        compressed[INNER_BLOCK_COUNT + 3],
    ]) as usize;

    // Read block sizes from table at 0x40
    let block_table_start = INNER_HEADER_MIN;
    let block_table_size = block_count * 4;
    let data_start = block_table_start + block_table_size;

    if compressed.len() < data_start {
        return Err(Error::DataTooShort {
            needed: data_start,
            actual: compressed.len(),
        });
    }

    // Parse block sizes (each is 4 bytes big-endian)
    let mut block_sizes = Vec::with_capacity(block_count);
    for i in 0..block_count {
        let off = block_table_start + i * 4;
        let size = u32::from_be_bytes([
            compressed[off],
            compressed[off + 1],
            compressed[off + 2],
            compressed[off + 3],
        ]) as usize;
        block_sizes.push(size);
    }

    // Decompress each block
    let mut output = Vec::with_capacity(decompressed_size);
    let mut current_offset = data_start;

    for (i, &block_size) in block_sizes.iter().enumerate() {
        if current_offset + block_size > compressed.len() {
            return Err(Error::DataTooShort {
                needed: current_offset + block_size,
                actual: compressed.len(),
            });
        }

        let block_data = &compressed[current_offset..current_offset + block_size];

        // Calculate expected decompressed size for this block
        // Last block may be smaller than BLOCK_DECOMP_SIZE
        let remaining = decompressed_size.saturating_sub(output.len());
        let block_decomp_size = remaining.min(BLOCK_DECOMP_SIZE);

        let block_output = decompressor
            .decompress_block(block_data, block_decomp_size)
            .map_err(|e| Error::Oodle(format!("block {}: {}", i, e)))?;

        output.extend_from_slice(&block_output);
        current_offset += block_size;
    }

    if output.len() != decompressed_size {
        return Err(Error::DecompressionSize {
            expected: decompressed_size,
            actual: output.len(),
        });
    }

    Ok(output)
}

/// Valid NCS version byte - only 0x01 is known to be valid
const VALID_VERSION: u8 = 0x01;

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
            // Only version 0x01 is valid - other versions are false positives
            if header.version != VALID_VERSION {
                continue;
            }

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

    /// Create a valid NCS header with given parameters
    fn make_ncs_header(
        version: u8,
        compression_flag: u32,
        decompressed: u32,
        compressed: u32,
    ) -> Vec<u8> {
        let mut data = vec![version];
        data.extend_from_slice(&NCS_MAGIC);
        data.extend_from_slice(&compression_flag.to_le_bytes());
        data.extend_from_slice(&decompressed.to_le_bytes());
        data.extend_from_slice(&compressed.to_le_bytes());
        data
    }

    #[test]
    fn test_header_size() {
        assert_eq!(HEADER_SIZE, 16);
    }

    #[test]
    fn test_inner_header_min() {
        assert_eq!(INNER_HEADER_MIN, 0x40);
    }

    #[test]
    fn test_header_parse_too_short() {
        let data = [0u8; 8];
        let result = Header::from_bytes(&data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::DataTooShort {
                needed: 16,
                actual: 8
            }
        ));
    }

    #[test]
    fn test_header_parse_invalid_magic() {
        let data = [
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let result = Header::from_bytes(&data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::InvalidNcsMagic(0x00, 0x00, 0x00)
        ));
    }

    #[test]
    fn test_header_parse_success() {
        let data = make_ncs_header(0x01, 1, 1000, 500);
        let header = Header::from_bytes(&data).unwrap();
        assert_eq!(header.version, 0x01);
        assert_eq!(header.compression_flag, 1);
        assert_eq!(header.decompressed_size, 1000);
        assert_eq!(header.compressed_size, 500);
    }

    #[test]
    fn test_header_is_compressed() {
        let compressed = Header::from_bytes(&make_ncs_header(1, 1, 100, 50)).unwrap();
        assert!(compressed.is_compressed());

        let uncompressed = Header::from_bytes(&make_ncs_header(1, 0, 100, 100)).unwrap();
        assert!(!uncompressed.is_compressed());
    }

    #[test]
    fn test_header_total_size() {
        let header = Header::from_bytes(&make_ncs_header(1, 0, 100, 50)).unwrap();
        assert_eq!(header.total_size(), HEADER_SIZE + 50);
    }

    #[test]
    fn test_decompress_payload_too_short() {
        // Header says 100 bytes of payload but we only provide 16 bytes
        let data = make_ncs_header(1, 0, 100, 100);
        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::DataTooShort { .. }));
    }

    #[test]
    fn test_decompress_uncompressed_data() {
        // Create uncompressed NCS data (compression_flag = 0)
        let payload = b"Hello, World!";
        let mut data = make_ncs_header(1, 0, payload.len() as u32, payload.len() as u32);
        data.extend_from_slice(payload);

        let result = decompress(&data).unwrap();
        assert_eq!(result, payload);
    }

    #[test]
    fn test_decompress_inner_too_short() {
        // Compressed flag = 1, but inner data is too short for Oodle header
        let mut data = make_ncs_header(1, 1, 100, 32);
        data.extend_from_slice(&[0u8; 32]); // Too short for INNER_HEADER_MIN (0x40)

        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::DataTooShort { needed: 64, .. }
        ));
    }

    #[test]
    fn test_decompress_inner_invalid_magic() {
        // Compressed with wrong inner magic
        let mut data = make_ncs_header(1, 1, 100, 0x50);
        // Wrong magic (should be 0xb7756362)
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        data.extend_from_slice(&[0u8; 0x4C]); // Padding to reach INNER_HEADER_MIN

        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::InvalidInnerMagic(0x00000000)
        ));
    }

    // Note: test_decompress_inner_raw_data removed - the "raw data" path was based
    // on incorrect format assumptions. All NCS files use multi-block Oodle compression.

    #[test]
    fn test_scan_empty_data() {
        let results = scan(&[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_no_ncs() {
        let data = b"Some random data without NCS magic";
        let results = scan(data);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_ncs_at_start() {
        // NCS magic at offset 0 should be skipped (needs version byte before it)
        let mut data = vec![];
        data.extend_from_slice(&NCS_MAGIC);
        data.extend_from_slice(&[0u8; 20]);

        let results = scan(&data);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_valid_ncs() {
        let mut data = vec![0u8; 10]; // Padding before NCS
        let ncs_data = make_ncs_header(1, 0, 8, 8);
        let ncs_start = data.len();
        data.extend_from_slice(&ncs_data);
        data.extend_from_slice(&[0u8; 8]); // Payload

        let results = scan(&data);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, ncs_start);
        assert_eq!(results[0].1.version, 1);
    }

    #[test]
    fn test_scan_skip_manifest() {
        // _NCS/ should be skipped (it's a manifest, not data)
        // The scan looks for "NCS" magic at bytes 1-3, so version byte is at offset-1
        // For "_NCS/", the NCS is at bytes 1-3, but there's underscore at byte 0
        // So when NCS magic is found at offset 1, start = 0, and data[start-1] check fails
        // Actually the check is: if start > 0 && data[start - 1] == b'_'
        // So we need proper structure

        // Build: [padding][_NCS/][rest]
        let mut data = vec![0u8; 5]; // Padding so start > 0
        data.push(b'_'); // This will be at start-1
                         // Now add version + NCS magic (the scan finds NCS at offset 7, start = 6)
        data.push(0x01); // Version byte (this is "start")
        data.extend_from_slice(&NCS_MAGIC); // NCS magic
        data.push(b'/');
        data.extend_from_slice(&[0u8; 20]);

        let results = scan(&data);
        assert!(results.is_empty(), "Expected empty but got {:?}", results);
    }

    #[test]
    fn test_scan_multiple_ncs() {
        let mut data = vec![];

        // First NCS chunk
        let ncs1 = make_ncs_header(1, 0, 4, 4);
        data.extend_from_slice(&ncs1);
        data.extend_from_slice(&[0u8; 4]);

        // Some padding
        data.extend_from_slice(&[0xFFu8; 20]);

        // Second NCS chunk (also version 1)
        let ncs2_start = data.len();
        let ncs2 = make_ncs_header(1, 0, 8, 8);
        data.extend_from_slice(&ncs2);
        data.extend_from_slice(&[0u8; 8]);

        let results = scan(&data);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[0].1.version, 1);
        assert_eq!(results[1].0, ncs2_start);
        assert_eq!(results[1].1.version, 1);
    }

    #[test]
    fn test_scan_ignores_other_versions() {
        let mut data = vec![];

        // Version 1 - should be found
        let ncs1 = make_ncs_header(1, 0, 4, 4);
        data.extend_from_slice(&ncs1);
        data.extend_from_slice(&[0u8; 4]);

        data.extend_from_slice(&[0xFFu8; 20]);

        // Version 2 - should be ignored
        let ncs2 = make_ncs_header(2, 0, 8, 8);
        data.extend_from_slice(&ncs2);
        data.extend_from_slice(&[0u8; 8]);

        let results = scan(&data);
        assert_eq!(results.len(), 1); // Only version 1
        assert_eq!(results[0].1.version, 1);
    }

    #[test]
    fn test_scan_ncs_truncated() {
        // NCS header says 100 bytes but file is truncated
        let ncs = make_ncs_header(1, 0, 100, 100);
        let mut data = vec![0u8; 5]; // Padding
        data.extend_from_slice(&ncs);
        // No payload - total_size exceeds data length

        let results = scan(&data);
        assert!(results.is_empty()); // Should not include truncated NCS
    }

    #[test]
    fn test_header_debug() {
        let header = Header::from_bytes(&make_ncs_header(1, 0, 100, 50)).unwrap();
        let debug = format!("{:?}", header);
        assert!(debug.contains("Header"));
        assert!(debug.contains("version"));
    }
}
