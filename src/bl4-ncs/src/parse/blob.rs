//! NCS decompressed payload header and header string extraction
//!
//! The decompressed NCS payload begins with a 16-byte header followed by
//! null-terminated header strings, then the body (type code table + data).

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
}
