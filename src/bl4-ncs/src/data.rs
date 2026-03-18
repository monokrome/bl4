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

use std::io::{self, Read};

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

// Inner header format flags offset (bytes 0x08-0x0b)
const INNER_FORMAT_FLAGS: usize = 0x08;

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

/// Block descriptor for streaming decompression
enum BlockDesc<'a> {
    Raw(&'a [u8]),
    Compressed { data: &'a [u8], decomp_size: usize },
}

/// Streaming reader that decompresses NCS data block-by-block.
///
/// Implements `std::io::Read`, lazily decompressing Oodle blocks on demand
/// rather than materializing the entire payload into memory at once.
pub struct DecompressReader<'a> {
    decompressor: &'a dyn OodleDecompressor,
    blocks: Vec<BlockDesc<'a>>,
    next_block: usize,
    buffer: Vec<u8>,
    buffer_pos: usize,
}

impl<'a> DecompressReader<'a> {
    fn load_next_block(&mut self) -> io::Result<bool> {
        if self.next_block >= self.blocks.len() {
            return Ok(false);
        }

        match &self.blocks[self.next_block] {
            BlockDesc::Raw(data) => {
                self.buffer.clear();
                self.buffer.extend_from_slice(data);
            }
            BlockDesc::Compressed { data, decomp_size } => {
                self.buffer = self.decompressor
                    .decompress_block(data, *decomp_size)
                    .map_err(|e| io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("block {}: {}", self.next_block, e),
                    ))?;
            }
        }

        self.buffer_pos = 0;
        self.next_block += 1;
        Ok(true)
    }
}

impl io::Read for DecompressReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.buffer_pos >= self.buffer.len() {
            if !self.load_next_block()? {
                return Ok(0);
            }
        }

        let available = &self.buffer[self.buffer_pos..];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.buffer_pos += n;
        Ok(n)
    }
}

/// Create a streaming decompression reader for NCS data.
///
/// Returns a `DecompressReader` that implements `std::io::Read` and lazily
/// decompresses Oodle blocks as bytes are consumed. Only one block (~256KB)
/// is held in memory at a time.
pub fn decompress_reader_with<'a>(
    data: &'a [u8],
    decompressor: &'a dyn OodleDecompressor,
) -> Result<DecompressReader<'a>> {
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
        return Ok(DecompressReader {
            decompressor,
            blocks: vec![BlockDesc::Raw(compressed)],
            next_block: 0,
            buffer: Vec::new(),
            buffer_pos: 0,
        });
    }

    build_compressed_reader(compressed, header.decompressed_size as usize, decompressor)
}

fn build_compressed_reader<'a>(
    compressed: &'a [u8],
    decompressed_size: usize,
    decompressor: &'a dyn OodleDecompressor,
) -> Result<DecompressReader<'a>> {
    if compressed.len() < INNER_HEADER_MIN {
        return Err(Error::DataTooShort {
            needed: INNER_HEADER_MIN,
            actual: compressed.len(),
        });
    }

    let inner_magic =
        u32::from_be_bytes([compressed[0], compressed[1], compressed[2], compressed[3]]);
    if inner_magic != OODLE_MAGIC {
        return Err(Error::InvalidInnerMagic(inner_magic));
    }

    let format_flags = u32::from_be_bytes([
        compressed[INNER_FORMAT_FLAGS],
        compressed[INNER_FORMAT_FLAGS + 1],
        compressed[INNER_FORMAT_FLAGS + 2],
        compressed[INNER_FORMAT_FLAGS + 3],
    ]);

    let blocks = if format_flags == 0 {
        build_single_block(compressed, decompressed_size)?
    } else {
        build_multi_block(compressed, decompressed_size)?
    };

    Ok(DecompressReader {
        decompressor,
        blocks,
        next_block: 0,
        buffer: Vec::new(),
        buffer_pos: 0,
    })
}

fn build_single_block<'a>(
    compressed: &'a [u8],
    decompressed_size: usize,
) -> Result<Vec<BlockDesc<'a>>> {
    let block_data = &compressed[INNER_HEADER_MIN..];

    if block_data.len() == decompressed_size {
        Ok(vec![BlockDesc::Raw(block_data)])
    } else {
        Ok(vec![BlockDesc::Compressed {
            data: block_data,
            decomp_size: decompressed_size,
        }])
    }
}

fn build_multi_block<'a>(
    compressed: &'a [u8],
    decompressed_size: usize,
) -> Result<Vec<BlockDesc<'a>>> {
    let block_count = u32::from_be_bytes([
        compressed[INNER_BLOCK_COUNT],
        compressed[INNER_BLOCK_COUNT + 1],
        compressed[INNER_BLOCK_COUNT + 2],
        compressed[INNER_BLOCK_COUNT + 3],
    ]) as usize;

    let block_table_start = INNER_HEADER_MIN;
    let block_table_size = block_count * 4;
    let data_start = block_table_start + block_table_size;

    if compressed.len() < data_start {
        return Err(Error::DataTooShort {
            needed: data_start,
            actual: compressed.len(),
        });
    }

    let mut blocks = Vec::with_capacity(block_count);
    let mut offset = data_start;
    let mut remaining = decompressed_size;

    for i in 0..block_count {
        let table_off = block_table_start + i * 4;
        let block_size = u32::from_be_bytes([
            compressed[table_off],
            compressed[table_off + 1],
            compressed[table_off + 2],
            compressed[table_off + 3],
        ]) as usize;

        if offset + block_size > compressed.len() {
            return Err(Error::DataTooShort {
                needed: offset + block_size,
                actual: compressed.len(),
            });
        }

        let block_decomp_size = remaining.min(BLOCK_DECOMP_SIZE);
        blocks.push(BlockDesc::Compressed {
            data: &compressed[offset..offset + block_size],
            decomp_size: block_decomp_size,
        });

        remaining = remaining.saturating_sub(block_decomp_size);
        offset += block_size;
    }

    Ok(blocks)
}

/// Decompress an NCS chunk using the default backend (oozextract)
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let backend = OozextractBackend::new();
    decompress_with(data, &backend)
}

/// Decompress an NCS chunk using a specific Oodle backend
pub fn decompress_with(data: &[u8], decompressor: &dyn OodleDecompressor) -> Result<Vec<u8>> {
    let header = Header::from_bytes(data)?;
    let expected = header.decompressed_size as usize;

    let mut reader = decompress_reader_with(data, decompressor)?;
    let mut buf = Vec::with_capacity(expected);
    reader.read_to_end(&mut buf)
        .map_err(|e| Error::Oodle(e.to_string()))?;

    if header.is_compressed() && buf.len() != expected {
        return Err(Error::DecompressionSize {
            expected,
            actual: buf.len(),
        });
    }

    Ok(buf)
}

/// Valid NCS version byte - only 0x01 is known to be valid
const VALID_VERSION: u8 = 0x01;

/// Scan for NCS data chunks in binary data
pub fn scan(data: &[u8]) -> Vec<(usize, Header)> {
    let finder = memmem::Finder::new(&NCS_MAGIC);
    let mut results = Vec::new();

    for offset in finder.find_iter(data) {
        if offset == 0 {
            continue;
        }

        let start = offset - 1;

        if start > 0 && data[start - 1] == b'_' {
            continue;
        }

        if let Ok(header) = Header::from_bytes(&data[start..]) {
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
    use std::io::Read;

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
        let data = make_ncs_header(1, 0, 100, 100);
        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::DataTooShort { .. }));
    }

    #[test]
    fn test_decompress_uncompressed_data() {
        let payload = b"Hello, World!";
        let mut data = make_ncs_header(1, 0, payload.len() as u32, payload.len() as u32);
        data.extend_from_slice(payload);

        let result = decompress(&data).unwrap();
        assert_eq!(result, payload);
    }

    #[test]
    fn test_decompress_inner_too_short() {
        let mut data = make_ncs_header(1, 1, 100, 32);
        data.extend_from_slice(&[0u8; 32]);

        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::DataTooShort { needed: 64, .. }
        ));
    }

    #[test]
    fn test_decompress_inner_invalid_magic() {
        let mut data = make_ncs_header(1, 1, 100, 0x50);
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        data.extend_from_slice(&[0u8; 0x4C]);

        let result = decompress(&data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::InvalidInnerMagic(0x00000000)
        ));
    }

    #[test]
    fn test_decompress_reader_uncompressed() {
        let payload = b"streaming test data";
        let mut data = make_ncs_header(1, 0, payload.len() as u32, payload.len() as u32);
        data.extend_from_slice(payload);

        let backend = OozextractBackend::new();
        let mut reader = decompress_reader_with(&data, &backend).unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, payload);
    }

    #[test]
    fn test_decompress_reader_partial_reads() {
        let payload = b"abcdefghijklmnopqrstuvwxyz";
        let mut data = make_ncs_header(1, 0, payload.len() as u32, payload.len() as u32);
        data.extend_from_slice(payload);

        let backend = OozextractBackend::new();
        let mut reader = decompress_reader_with(&data, &backend).unwrap();

        let mut buf = [0u8; 4];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf, b"abcd");

        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf, b"efgh");
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
        let mut data = vec![];
        data.extend_from_slice(&NCS_MAGIC);
        data.extend_from_slice(&[0u8; 20]);

        let results = scan(&data);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_valid_ncs() {
        let mut data = vec![0u8; 10];
        let ncs_data = make_ncs_header(1, 0, 8, 8);
        let ncs_start = data.len();
        data.extend_from_slice(&ncs_data);
        data.extend_from_slice(&[0u8; 8]);

        let results = scan(&data);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, ncs_start);
        assert_eq!(results[0].1.version, 1);
    }

    #[test]
    fn test_scan_skip_manifest() {
        let mut data = vec![0u8; 5];
        data.push(b'_');
        data.push(0x01);
        data.extend_from_slice(&NCS_MAGIC);
        data.push(b'/');
        data.extend_from_slice(&[0u8; 20]);

        let results = scan(&data);
        assert!(results.is_empty(), "Expected empty but got {:?}", results);
    }

    #[test]
    fn test_scan_multiple_ncs() {
        let mut data = vec![];

        let ncs1 = make_ncs_header(1, 0, 4, 4);
        data.extend_from_slice(&ncs1);
        data.extend_from_slice(&[0u8; 4]);

        data.extend_from_slice(&[0xFFu8; 20]);

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

        let ncs1 = make_ncs_header(1, 0, 4, 4);
        data.extend_from_slice(&ncs1);
        data.extend_from_slice(&[0u8; 4]);

        data.extend_from_slice(&[0xFFu8; 20]);

        let ncs2 = make_ncs_header(2, 0, 8, 8);
        data.extend_from_slice(&ncs2);
        data.extend_from_slice(&[0u8; 8]);

        let results = scan(&data);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.version, 1);
    }

    #[test]
    fn test_scan_ncs_truncated() {
        let ncs = make_ncs_header(1, 0, 100, 100);
        let mut data = vec![0u8; 5];
        data.extend_from_slice(&ncs);

        let results = scan(&data);
        assert!(results.is_empty());
    }

    #[test]
    fn test_header_debug() {
        let header = Header::from_bytes(&make_ncs_header(1, 0, 100, 50)).unwrap();
        let debug = format!("{:?}", header);
        assert!(debug.contains("Header"));
        assert!(debug.contains("version"));
    }
}
