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

    /// Create a valid NCS header with given parameters
    fn make_ncs_header(version: u8, compression_flag: u32, decompressed: u32, compressed: u32) -> Vec<u8> {
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
        assert!(matches!(result.unwrap_err(), Error::DataTooShort { needed: 16, actual: 8 }));
    }

    #[test]
    fn test_header_parse_invalid_magic() {
        let data = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = Header::from_bytes(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidNcsMagic(0x00, 0x00, 0x00)));
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
        data.extend_from_slice(&[0u8; 32]);  // Too short for INNER_HEADER_MIN (0x40)

        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::DataTooShort { needed: 64, .. }));
    }

    #[test]
    fn test_decompress_inner_invalid_magic() {
        // Compressed with wrong inner magic
        let mut data = make_ncs_header(1, 1, 100, 0x50);
        // Wrong magic (should be 0xb7756362)
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        data.extend_from_slice(&[0u8; 0x4C]);  // Padding to reach INNER_HEADER_MIN

        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidInnerMagic(0x00000000)));
    }

    #[test]
    fn test_decompress_inner_raw_data() {
        // Compression type = 0 means raw data starts at INNER_RAW_DATA_START
        let raw_payload = b"Raw payload data";
        let inner_size = INNER_RAW_DATA_START + raw_payload.len();

        let mut data = make_ncs_header(1, 1, raw_payload.len() as u32, inner_size as u32);

        // Build inner header with Oodle magic
        let mut inner = vec![0u8; INNER_RAW_DATA_START];
        inner[0..4].copy_from_slice(&OODLE_MAGIC.to_be_bytes());
        inner[INNER_COMPRESSION_TYPE] = 0;  // Raw data, no compression
        inner.extend_from_slice(raw_payload);

        data.extend_from_slice(&inner);

        let result = decompress(&data).unwrap();
        assert_eq!(result, raw_payload);
    }

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
        let mut data = vec![0u8; 10];  // Padding before NCS
        let ncs_data = make_ncs_header(1, 0, 8, 8);
        let ncs_start = data.len();
        data.extend_from_slice(&ncs_data);
        data.extend_from_slice(&[0u8; 8]);  // Payload

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
        let mut data = vec![0u8; 5];  // Padding so start > 0
        data.push(b'_');              // This will be at start-1
        // Now add version + NCS magic (the scan finds NCS at offset 7, start = 6)
        data.push(0x01);              // Version byte (this is "start")
        data.extend_from_slice(&NCS_MAGIC);  // NCS magic
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

        // Second NCS chunk
        let ncs2_start = data.len();
        let ncs2 = make_ncs_header(2, 0, 8, 8);
        data.extend_from_slice(&ncs2);
        data.extend_from_slice(&[0u8; 8]);

        let results = scan(&data);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[0].1.version, 1);
        assert_eq!(results[1].0, ncs2_start);
        assert_eq!(results[1].1.version, 2);
    }

    #[test]
    fn test_scan_ncs_truncated() {
        // NCS header says 100 bytes but file is truncated
        let ncs = make_ncs_header(1, 0, 100, 100);
        let mut data = vec![0u8; 5];  // Padding
        data.extend_from_slice(&ncs);
        // No payload - total_size exceeds data length

        let results = scan(&data);
        assert!(results.is_empty());  // Should not include truncated NCS
    }

    #[test]
    fn test_header_debug() {
        let header = Header::from_bytes(&make_ncs_header(1, 0, 100, 50)).unwrap();
        let debug = format!("{:?}", header);
        assert!(debug.contains("Header"));
        assert!(debug.contains("version"));
    }
}
