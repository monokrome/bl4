//! Type code table parsing
//!
//! Parses the TypeCodeBodyHeader (type codes + bit matrix) and the three
//! string blocks (value_strings, value_kinds, key_strings) from the body
//! section of decompressed NCS data.

use super::blob::parse_null_terminated_strings;

/// Map a type code character to its global bit position.
///
/// Reference: `try_get_global_type_bit()` in ncs_type_code_table.cpp
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
    let type_codes = std::str::from_utf8(&body[pos..pos + tc_len]).ok()?.to_string();
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
            let byte_idx = bit_pos >> 3;
            let bit_in_byte = bit_pos & 7;
            if byte_idx < body.len() && ((body[byte_idx] >> bit_in_byte) & 1) != 0 {
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
    let flags = u32::from_le_bytes([
        buf[pos + 4],
        buf[pos + 5],
        buf[pos + 6],
        buf[pos + 7],
    ]);
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
    if strings.len() < target {
        try_repair_merged_numeric_prefix_strings(&mut strings, target);
    }

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

/// Repair strings that were merged due to missing null terminators
///
/// Some strings get concatenated like "123FooBar" where "123" and "FooBar"
/// should be separate entries. Split at digit→alpha boundaries.
fn try_repair_merged_numeric_prefix_strings(strings: &mut Vec<String>, target_count: usize) {
    for _ in 0..64 {
        if strings.len() >= target_count {
            break;
        }

        let mut candidate = None;
        let mut split_byte_pos = 0;

        for (i, s) in strings.iter().enumerate() {
            if s.is_empty() {
                continue;
            }
            let chars: Vec<char> = s.chars().collect();
            let mut p = 0;
            while p < chars.len() && chars[p].is_ascii_digit() {
                p += 1;
            }
            if p < 2 || p >= chars.len() {
                continue;
            }
            if !chars[p].is_ascii_alphabetic() {
                continue;
            }
            // Found a split point
            let byte_pos = s.char_indices().nth(p).map(|(idx, _)| idx).unwrap_or(s.len());
            candidate = Some(i);
            split_byte_pos = byte_pos;
            break;
        }

        if let Some(idx) = candidate {
            let original = strings[idx].clone();
            let left = original[..split_byte_pos].to_string();
            let right = original[split_byte_pos..].to_string();
            strings[idx] = left;
            strings.insert(idx + 1, right);
        } else {
            break;
        }
    }
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
    fn test_string_repair() {
        let mut strings = vec!["123Hello".to_string(), "world".to_string()];
        try_repair_merged_numeric_prefix_strings(&mut strings, 3);
        assert_eq!(strings, vec!["123", "Hello", "world"]);
    }

    #[test]
    fn test_string_repair_no_split_needed() {
        let mut strings = vec!["hello".to_string(), "world".to_string()];
        try_repair_merged_numeric_prefix_strings(&mut strings, 2);
        assert_eq!(strings, vec!["hello", "world"]);
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
}
