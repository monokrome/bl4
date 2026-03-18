//! NCS decompressed payload header and header string extraction
//!
//! The decompressed NCS payload begins with a 16-byte header followed by
//! null-terminated header strings, then the body (type code table + data).

use std::io::Read;

/// Blob header at start of decompressed NCS data (16 bytes)
#[derive(Debug, Clone, Copy)]
pub struct BlobHeader {
    pub entry_count: u32,
    pub flags: u32,
    pub string_bytes: u32,
    pub reserved: u32,
}

impl BlobHeader {
    pub const SIZE: usize = 16;

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        let header = Self {
            entry_count: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            flags: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            string_bytes: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            reserved: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        };

        if header.reserved != 0 {
            return None;
        }
        if header.entry_count > 100_000 {
            return None;
        }
        if header.string_bytes > 10_000_000 {
            return None;
        }

        Some(header)
    }

    /// Parse blob header from a stream (reads exactly 16 bytes)
    pub fn from_reader(reader: &mut impl Read) -> Option<Self> {
        let mut buf = [0u8; Self::SIZE];
        reader.read_exact(&mut buf).ok()?;
        Self::parse(&buf)
    }

    pub fn string_table_offset(&self) -> usize {
        Self::SIZE
    }

    pub fn body_offset(&self) -> usize {
        Self::SIZE + self.string_bytes as usize
    }
}

/// Extract null-terminated header strings from the string block
pub fn extract_header_strings(data: &[u8], blob: &BlobHeader) -> Vec<String> {
    let start = blob.string_table_offset();
    let end = blob.body_offset();

    if end > data.len() {
        return Vec::new();
    }

    let block = &data[start..end];
    parse_null_terminated_strings(block)
}

/// Read header strings from a stream (reads exactly `string_bytes` bytes)
pub fn read_header_strings(reader: &mut impl Read, blob: &BlobHeader) -> Option<Vec<String>> {
    let len = blob.string_bytes as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    Some(parse_null_terminated_strings(&buf))
}

/// Parse null-terminated strings from a byte block
pub fn parse_null_terminated_strings(block: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;

    while pos < block.len() {
        let start = pos;
        while pos < block.len() && block[pos] != 0 {
            pos += 1;
        }
        if let Ok(s) = std::str::from_utf8(&block[start..pos]) {
            out.push(s.to_string());
        }
        if pos >= block.len() {
            break;
        }
        pos += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_header_parse() {
        let mut data = vec![0u8; 32];
        // entry_count = 5
        data[0] = 5;
        // flags = 0
        // string_bytes = 10
        data[8] = 10;
        // reserved = 0

        let header = BlobHeader::parse(&data).unwrap();
        assert_eq!(header.entry_count, 5);
        assert_eq!(header.flags, 0);
        assert_eq!(header.string_bytes, 10);
        assert_eq!(header.reserved, 0);
        assert_eq!(header.string_table_offset(), 16);
        assert_eq!(header.body_offset(), 26);
    }

    #[test]
    fn test_blob_header_rejects_nonzero_reserved() {
        let mut data = vec![0u8; 16];
        data[12] = 1; // reserved != 0
        assert!(BlobHeader::parse(&data).is_none());
    }

    #[test]
    fn test_parse_null_terminated_strings() {
        let data = b"hello\0world\0test\0";
        let strings = parse_null_terminated_strings(data);
        assert_eq!(strings, vec!["hello", "world", "test"]);
    }

    #[test]
    fn test_parse_null_terminated_strings_empty() {
        let data = b"";
        let strings = parse_null_terminated_strings(data);
        assert!(strings.is_empty());
    }

    #[test]
    fn test_blob_header_from_reader() {
        let mut data = vec![0u8; 32];
        data[0] = 5; // entry_count
        data[8] = 10; // string_bytes

        let mut cursor = std::io::Cursor::new(&data);
        let header = BlobHeader::from_reader(&mut cursor).unwrap();
        assert_eq!(header.entry_count, 5);
        assert_eq!(header.string_bytes, 10);
        // Cursor should have advanced past the header
        assert_eq!(cursor.position(), 16);
    }

    #[test]
    fn test_read_header_strings() {
        let blob = BlobHeader {
            entry_count: 1,
            flags: 0,
            string_bytes: 12,
            reserved: 0,
        };
        let data = b"hello\0world\0";
        let mut cursor = std::io::Cursor::new(&data[..]);
        let strings = read_header_strings(&mut cursor, &blob).unwrap();
        assert_eq!(strings, vec!["hello", "world"]);
    }

    #[test]
    fn test_from_reader_matches_parse() {
        let mut data = vec![0u8; 32];
        data[0] = 3; // entry_count
        data[8] = 20; // string_bytes

        let from_slice = BlobHeader::parse(&data).unwrap();
        let mut cursor = std::io::Cursor::new(&data);
        let from_reader = BlobHeader::from_reader(&mut cursor).unwrap();

        assert_eq!(from_slice.entry_count, from_reader.entry_count);
        assert_eq!(from_slice.flags, from_reader.flags);
        assert_eq!(from_slice.string_bytes, from_reader.string_bytes);
        assert_eq!(from_slice.reserved, from_reader.reserved);
    }
}
