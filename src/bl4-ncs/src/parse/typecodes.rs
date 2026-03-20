//! Type code table parsing
//!
//! Parses the TypeCodeBodyHeader (type codes + bit matrix) and the three
//! string blocks (value_strings, value_kinds, key_strings) from the body
//! section of decompressed NCS data.

use std::io::Read;

use super::blob::parse_null_terminated_strings;

fn read_bit(buf: &[u8], bit_pos: usize) -> bool {
    let byte_idx = bit_pos >> 3;
    let bit_in_byte = bit_pos & 7;
    byte_idx < buf.len() && ((buf[byte_idx] >> bit_in_byte) & 1) != 0
}

/// Map a type code character to its global bit position.
fn global_type_bit(ch: char) -> Option<u8> {
    match ch {
        'a' => Some(0),
        'b' => Some(1),
        'c' => Some(2),
        'd' => Some(3),
        'e' => Some(4),
        'f' => Some(5),
        'g' => Some(6),
        'h' => Some(7),
        'i' => Some(8),
        'j' => Some(9),
        'k' => Some(10),
        'l' => Some(11),
        'm' => Some(24),
        _ => None,
    }
}

/// Parsed type code body header
#[derive(Debug, Clone)]
pub struct TypeCodeHeader {
    pub type_code_count: u8,
    pub type_codes: String,
    pub type_index_count: u16,
    pub row_flags: Vec<u32>,
}

/// A string block from the body section
#[derive(Debug, Clone)]
pub struct StringBlock {
    pub declared_count: u32,
    pub flags: u32,
    pub byte_length: u64,
    pub strings: Vec<String>,
}

/// Complete parsed type code table with all string tables
#[derive(Debug, Clone)]
pub struct TypeCodeTable {
    pub header: TypeCodeHeader,
    pub value_strings: Vec<String>,
    pub value_strings_declared_count: u32,
    pub value_strings_flags: u32,
    pub value_kinds: Vec<String>,
    pub value_kinds_declared_count: u32,
    pub key_strings: Vec<String>,
    pub key_strings_declared_count: u32,
    pub data_offset: usize,
}

/// Parse the TypeCodeTable from the body section
pub fn parse_type_code_table(body: &[u8]) -> Option<TypeCodeTable> {
    if body.len() < 3 {
        return None;
    }

    let type_code_count = body[0];
    let type_index_count = u16::from_le_bytes([body[1], body[2]]);
    let mut pos = 3;

    if type_code_count == 0 || type_code_count > 64 {
        return None;
    }

    let tc_len = type_code_count as usize;
    if pos + tc_len > body.len() {
        return None;
    }
    let type_codes = std::str::from_utf8(&body[pos..pos + tc_len])
        .ok()?
        .to_string();
    pos += tc_len;

    let (row_flags, next_pos) = parse_bit_matrix(body, pos, &type_codes, type_index_count)?;
    pos = next_pos;

    let (value_block, next_pos) = read_string_block(body, pos)?;
    pos = next_pos;

    let kinds_block = try_read_string_block(body, &mut pos);
    let keys_block = try_read_string_block(body, &mut pos);

    let header = TypeCodeHeader {
        type_code_count,
        type_codes,
        type_index_count,
        row_flags,
    };

    Some(TypeCodeTable {
        header,
        value_strings_declared_count: value_block.declared_count,
        value_strings_flags: value_block.flags,
        value_strings: value_block.strings,
        value_kinds_declared_count: kinds_block.as_ref().map(|b| b.declared_count).unwrap_or(0),
        value_kinds: kinds_block.map(|b| b.strings).unwrap_or_default(),
        key_strings_declared_count: keys_block.as_ref().map(|b| b.declared_count).unwrap_or(0),
        key_strings: keys_block.map(|b| b.strings).unwrap_or_default(),
        data_offset: pos,
    })
}

/// Parse the TypeCodeTable from a stream
///
/// Reads the type code header, bit matrix, and up to 3 string blocks
/// sequentially from the reader. Returns the parsed table and sets
/// `data_offset` to the total bytes consumed.
pub fn parse_type_code_table_from_reader(reader: &mut impl Read) -> Option<TypeCodeTable> {
    let mut header_buf = [0u8; 3];
    reader.read_exact(&mut header_buf).ok()?;
    let mut bytes_read: usize = 3;

    let type_code_count = header_buf[0];
    let type_index_count = u16::from_le_bytes([header_buf[1], header_buf[2]]);

    if type_code_count == 0 || type_code_count > 64 {
        return None;
    }

    let tc_len = type_code_count as usize;
    let mut tc_buf = vec![0u8; tc_len];
    reader.read_exact(&mut tc_buf).ok()?;
    bytes_read += tc_len;
    let type_codes = std::str::from_utf8(&tc_buf).ok()?.to_string();

    let row_flags =
        parse_bit_matrix_from_reader(reader, &type_codes, type_index_count, &mut bytes_read)?;

    let value_block = read_string_block_from_reader(reader, &mut bytes_read)?;
    let kinds_block = read_string_block_from_reader(reader, &mut bytes_read);
    let keys_block = read_string_block_from_reader(reader, &mut bytes_read);

    let header = TypeCodeHeader {
        type_code_count,
        type_codes,
        type_index_count,
        row_flags,
    };

    Some(TypeCodeTable {
        header,
        value_strings_declared_count: value_block.declared_count,
        value_strings_flags: value_block.flags,
        value_strings: value_block.strings,
        value_kinds_declared_count: kinds_block.as_ref().map(|b| b.declared_count).unwrap_or(0),
        value_kinds: kinds_block.map(|b| b.strings).unwrap_or_default(),
        key_strings_declared_count: keys_block.as_ref().map(|b| b.declared_count).unwrap_or(0),
        key_strings: keys_block.map(|b| b.strings).unwrap_or_default(),
        data_offset: bytes_read,
    })
}

/// Parse bit matrix from a stream, updating bytes_read
fn parse_bit_matrix_from_reader(
    reader: &mut impl Read,
    type_codes: &str,
    type_index_count: u16,
    bytes_read: &mut usize,
) -> Option<Vec<u32>> {
    let col_count = type_codes.len();
    let matrix_byte_count = (col_count * type_index_count as usize).div_ceil(8);

    let mut matrix_buf = vec![0u8; matrix_byte_count];
    reader.read_exact(&mut matrix_buf).ok()?;

    let type_code_chars: Vec<char> = type_codes.chars().collect();
    let mut row_flags = Vec::with_capacity(type_index_count as usize);
    let mut bit_pos: usize = 0;

    for _ in 0..type_index_count {
        let mut flags = 0u32;
        for (col, &ch) in type_code_chars.iter().enumerate() {
            if read_bit(&matrix_buf, bit_pos) {
                let shift = global_type_bit(ch).unwrap_or(col as u8);
                flags |= 1u32 << shift;
            }
            bit_pos += 1;
        }
        row_flags.push(flags);
    }

    // Account for byte-aligned matrix size
    *bytes_read += matrix_byte_count;
    Some(row_flags)
}

/// Read a string block from a stream: 16-byte header + string data
fn read_string_block_from_reader(
    reader: &mut impl Read,
    bytes_read: &mut usize,
) -> Option<StringBlock> {
    let mut hdr = [0u8; 16];
    reader.read_exact(&mut hdr).ok()?;

    let declared_count = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    let flags = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
    let byte_length = u64::from_le_bytes([
        hdr[8], hdr[9], hdr[10], hdr[11], hdr[12], hdr[13], hdr[14], hdr[15],
    ]);

    let byte_len = byte_length as usize;
    let mut string_buf = vec![0u8; byte_len];
    reader.read_exact(&mut string_buf).ok()?;

    let mut strings = parse_null_terminated_strings(&string_buf);
    let target = declared_count as usize;
    if strings.len() > target {
        strings.truncate(target);
    }
    while strings.len() < target {
        strings.push(String::new());
    }

    *bytes_read += 16 + byte_len;
    Some(StringBlock {
        declared_count,
        flags,
        byte_length,
        strings,
    })
}

/// Try to read an optional string block, advancing pos on success
fn try_read_string_block(body: &[u8], pos: &mut usize) -> Option<StringBlock> {
    if *pos + 16 > body.len() {
        return None;
    }
    match read_string_block(body, *pos) {
        Some((block, next)) => {
            *pos = next;
            Some(block)
        }
        None => None,
    }
}

/// Parse the bit matrix and compute row_flags from type codes
fn parse_bit_matrix(
    body: &[u8],
    pos: usize,
    type_codes: &str,
    type_index_count: u16,
) -> Option<(Vec<u32>, usize)> {
    let col_count = type_codes.len();
    let matrix_byte_count = (col_count * type_index_count as usize).div_ceil(8);

    if pos + matrix_byte_count > body.len() {
        return None;
    }

    let mut bit_pos = pos * 8;
    let type_code_chars: Vec<char> = type_codes.chars().collect();
    let mut row_flags = Vec::with_capacity(type_index_count as usize);

    for _ in 0..type_index_count {
        let mut flags = 0u32;
        for (col, &ch) in type_code_chars.iter().enumerate() {
            if read_bit(body, bit_pos) {
                let shift = global_type_bit(ch).unwrap_or(col as u8);
                flags |= 1u32 << shift;
            }
            bit_pos += 1;
        }
        row_flags.push(flags);
    }

    // Align to byte boundary
    let aligned = bit_pos.div_ceil(8) * 8;
    Some((row_flags, aligned / 8))
}

/// Read a string block: declared_count(u32) + flags(u32) + byte_length(u64) + strings
fn read_string_block(buf: &[u8], pos: usize) -> Option<(StringBlock, usize)> {
    if pos + 16 > buf.len() {
        return None;
    }

    let declared_count = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
    let flags = u32::from_le_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]);
    let byte_length = u64::from_le_bytes([
        buf[pos + 8],
        buf[pos + 9],
        buf[pos + 10],
        buf[pos + 11],
        buf[pos + 12],
        buf[pos + 13],
        buf[pos + 14],
        buf[pos + 15],
    ]);

    if byte_length > (buf.len() - (pos + 16)) as u64 {
        return None;
    }

    let byte_len = byte_length as usize;
    let data_start = pos + 16;
    let mut strings = parse_null_terminated_strings(&buf[data_start..data_start + byte_len]);

    let target = declared_count as usize;
    if strings.len() > target {
        strings.truncate(target);
    }
    while strings.len() < target {
        strings.push(String::new());
    }

    let block = StringBlock {
        declared_count,
        flags,
        byte_length,
        strings,
    };

    Some((block, pos + 16 + byte_len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_type_bit() {
        assert_eq!(global_type_bit('a'), Some(0));
        assert_eq!(global_type_bit('b'), Some(1));
        assert_eq!(global_type_bit('j'), Some(9));
        assert_eq!(global_type_bit('m'), Some(24));
        assert_eq!(global_type_bit('z'), None);
    }

    #[test]
    fn test_read_string_block() {
        // Build a string block: declared=2, flags=0, byte_length=12, "hello\0world\0"
        let mut data = vec![0u8; 28];
        data[0] = 2; // declared_count
                     // flags = 0
        data[8] = 12; // byte_length
        data[16..28].copy_from_slice(b"hello\0world\0");

        let (block, next_pos) = read_string_block(&data, 0).unwrap();
        assert_eq!(block.declared_count, 2);
        assert_eq!(block.strings, vec!["hello", "world"]);
        assert_eq!(next_pos, 28);
    }

    #[test]
    fn test_read_string_block_from_reader() {
        let mut data = vec![0u8; 28];
        data[0] = 2; // declared_count
        data[8] = 12; // byte_length
        data[16..28].copy_from_slice(b"hello\0world\0");

        let mut bytes_read = 0;
        let mut cursor = std::io::Cursor::new(&data);
        let block = read_string_block_from_reader(&mut cursor, &mut bytes_read).unwrap();
        assert_eq!(block.declared_count, 2);
        assert_eq!(block.strings, vec!["hello", "world"]);
        assert_eq!(bytes_read, 28);
    }

    /// Build a minimal valid type code table body for testing
    fn build_test_body() -> Vec<u8> {
        let mut body = Vec::new();

        // Header: type_code_count=2, type_index_count=3
        body.push(2); // type_code_count
        body.extend_from_slice(&3u16.to_le_bytes()); // type_index_count

        // Type codes: "ab"
        body.extend_from_slice(b"ab");

        // Bit matrix: 3 rows x 2 cols = 6 bits = 1 byte
        // Row 0: a=1, b=0 → flags=0x01
        // Row 1: a=0, b=1 → flags=0x02
        // Row 2: a=1, b=1 → flags=0x03
        body.push(0b00_11_01_01); // bits: row0(1,0), row1(0,1), row2(1,1), pad(0,0)

        // Value strings block: declared=1, flags=0, byte_length=4, "foo\0"
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&4u64.to_le_bytes());
        body.extend_from_slice(b"foo\0");

        body
    }

    #[test]
    fn test_parse_type_code_table_from_reader_matches_slice() {
        let body = build_test_body();

        let from_slice = parse_type_code_table(&body).unwrap();

        let mut cursor = std::io::Cursor::new(&body);
        let from_reader = parse_type_code_table_from_reader(&mut cursor).unwrap();

        assert_eq!(
            from_slice.header.type_code_count,
            from_reader.header.type_code_count
        );
        assert_eq!(from_slice.header.type_codes, from_reader.header.type_codes);
        assert_eq!(
            from_slice.header.type_index_count,
            from_reader.header.type_index_count
        );
        assert_eq!(from_slice.header.row_flags, from_reader.header.row_flags);
        assert_eq!(from_slice.value_strings, from_reader.value_strings);
        assert_eq!(
            from_slice.value_strings_declared_count,
            from_reader.value_strings_declared_count
        );
        assert_eq!(from_slice.data_offset, from_reader.data_offset);
    }
}
