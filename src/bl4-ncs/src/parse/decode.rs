//! Table data decode loop
//!
//! Implements the NCS table data decode algorithm: tables → records → entries.

use crate::bit_reader::{bit_width, BitReader};
use crate::document::{DepEntry, Document, Entry, Record, Table, Tag, Value};
use crate::parse::remap::FixedWidthIntArray;
use std::collections::HashMap;

/// All string tables and precomputed bit widths needed during decoding
struct DecodeContext<'a> {
    value_strings: &'a [String],
    value_kinds: &'a [String],
    key_strings: &'a [String],
    header_index_bits: u8,
    value_index_bits: u8,
    value_kind_bits: u8,
    key_index_bits: u8,
    type_index_bits: u8,
    row_flags: &'a [u32],
}

/// Per-table remap and dependency state
struct TableContext<'a> {
    pair_remap: Option<&'a FixedWidthIntArray>,
    value_remap: Option<&'a FixedWidthIntArray>,
    dep_names: Vec<String>,
    dep_index_bits: u8,
}

/// Resolve a remap: use the remap's bit width if active, otherwise the default
fn remap_index(remap: Option<&FixedWidthIntArray>, raw: u32, default_bits: u8) -> (u8, u32) {
    match remap {
        Some(r) if r.is_active() => (r.index_bit_width, r.remap(raw).unwrap_or(raw)),
        _ => (default_bits, raw),
    }
}

/// Read a key string using the pair_vec remap
fn read_pair_vec_string(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    remap: Option<&FixedWidthIntArray>,
) -> Option<String> {
    let (bits, _) = remap_index(remap, 0, ctx.key_index_bits);
    let raw_index = reader.read_bits(bits)?;
    let (_, mapped) = remap_index(remap, raw_index, ctx.key_index_bits);

    if (mapped as usize) < ctx.key_strings.len() {
        Some(ctx.key_strings[mapped as usize].clone())
    } else {
        Some(format!("<key:{}>", mapped))
    }
}

/// Read a leaf value string using the value_string remap + kind
fn read_value(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    value_remap: Option<&FixedWidthIntArray>,
) -> Option<String> {
    let (bits, _) = remap_index(value_remap, 0, ctx.value_index_bits);
    let raw_index = reader.read_bits(bits)?;
    let kind_index = reader.read_bits(ctx.value_kind_bits)? as usize;
    let (_, mapped) = remap_index(value_remap, raw_index, ctx.value_index_bits);
    let value_index = mapped as usize;

    let value = ctx
        .value_strings
        .get(value_index)
        .cloned()
        .unwrap_or_default();

    let type_name = ctx.value_kinds.get(kind_index).map(|s| s.as_str()).unwrap_or("");

    if type_name.is_empty() {
        Some(value)
    } else {
        Some(format!("{}'{}'", type_name, value))
    }
}

/// Decode a node recursively based on type flags
///
/// Reference: decode_node() in ncs_table_data_decoder.cpp
fn decode_node(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    tctx: &TableContext,
    record_end_bit: usize,
) -> Option<Value> {
    let type_index = reader.read_bits(ctx.type_index_bits)? as usize;
    let flags = ctx.row_flags.get(type_index).copied().unwrap_or(0);
    let kind = (flags & 3) as u8;
    let has_self_key = kind == 3 || (flags & 0x80) != 0;

    let self_key = if has_self_key {
        read_pair_vec_string(reader, ctx, tctx.pair_remap)?
    } else {
        String::new()
    };

    let value = decode_node_value(reader, ctx, tctx, record_end_bit, kind)?;
    wrap_with_self_key(self_key, value)
}

/// Decode the value portion of a node based on kind
fn decode_node_value(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    tctx: &TableContext,
    record_end_bit: usize,
    kind: u8,
) -> Option<Value> {
    match kind {
        0 => Some(Value::Null),
        1 => Some(Value::Leaf(read_value(reader, ctx, tctx.value_remap)?)),
        2 => {
            let mut arr = Vec::new();
            while reader.position() < record_end_bit {
                if !reader.read_bit()? {
                    break;
                }
                arr.push(decode_node(reader, ctx, tctx, record_end_bit)?);
            }
            Some(Value::Array(arr))
        }
        3 => {
            let mut map = HashMap::new();
            while reader.position() < record_end_bit {
                if !reader.read_bit()? {
                    break;
                }
                let k = read_pair_vec_string(reader, ctx, tctx.pair_remap)?;
                let v = decode_node(reader, ctx, tctx, record_end_bit)?;
                map.insert(k, v);
            }
            Some(Value::Map(map))
        }
        _ => Some(Value::Null),
    }
}

/// Wrap a value with a self_key if present and non-trivial
fn wrap_with_self_key(self_key: String, value: Value) -> Option<Value> {
    if !self_key.is_empty() && !self_key.eq_ignore_ascii_case("none") {
        let mut wrapper = HashMap::new();
        wrapper.insert(self_key, value);
        Some(Value::Map(wrapper))
    } else {
        Some(value)
    }
}

/// Read a packed name list (used by tags d/e/f)
fn read_packed_name_list(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    pair_remap: Option<&FixedWidthIntArray>,
) -> Option<Vec<String>> {
    let mut list = Vec::new();
    for _ in 0..4096 {
        let s = read_pair_vec_string(reader, ctx, pair_remap)?;
        if s.is_empty() || s.eq_ignore_ascii_case("none") {
            break;
        }
        list.push(s);
    }
    Some(list)
}

/// Read an entry or dep_entry value based on a 2-bit opcode
fn decode_op_value(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    tctx: &TableContext,
    record_end_bit: usize,
    op: u32,
) -> Option<Value> {
    match op {
        1 => Some(Value::Null),
        2 => decode_node(reader, ctx, tctx, record_end_bit),
        3 => {
            let ref_str = read_pair_vec_string(reader, ctx, tctx.pair_remap)?;
            Some(Value::Ref { r#ref: ref_str })
        }
        _ => Some(Value::Null),
    }
}

/// Parse record tags until 'z' marker, capturing metadata
fn parse_tags(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    tctx: &TableContext,
    record_end_bit: usize,
) -> Vec<Tag> {
    let mut tags = Vec::new();

    while reader.position() + 8 <= record_end_bit {
        let Some(tag_byte) = reader.read_bits(8) else {
            break;
        };
        let tag_byte = tag_byte as u8;
        if tag_byte == b'z' {
            break;
        }

        let tag = match tag_byte {
            b'a' => read_pair_vec_string(reader, ctx, tctx.pair_remap)
                .map(|pair| Tag::KeyName { pair }),
            b'b' => reader.read_bits(32).map(|value| Tag::U32 { value }),
            b'c' => reader.read_bits(32).map(|u32_value| {
                let f32_value = f32::from_bits(u32_value);
                Tag::F32 {
                    u32_value,
                    f32_value,
                }
            }),
            b'd' => read_packed_name_list(reader, ctx, tctx.pair_remap)
                .map(|list| Tag::NameListD { list }),
            b'e' => read_packed_name_list(reader, ctx, tctx.pair_remap)
                .map(|list| Tag::NameListE { list }),
            b'f' => read_packed_name_list(reader, ctx, tctx.pair_remap)
                .map(|list| Tag::NameListF { list }),
            b'p' => decode_node(reader, ctx, tctx, record_end_bit)
                .map(|variant| Tag::Variant { variant }),
            _ => break,
        };

        match tag {
            Some(t) => tags.push(t),
            None => break,
        }
    }

    tags
}

/// Parse entries from a record's entry section
fn parse_entries(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    tctx: &TableContext,
    record_end_bit: usize,
) -> Vec<Entry> {
    let mut entries = Vec::new();

    while reader.position() + 2 <= record_end_bit {
        let Some(op) = reader.read_bits(2) else {
            break;
        };
        if op == 0 {
            break;
        }

        let Some(key) = read_pair_vec_string(reader, ctx, tctx.pair_remap) else {
            break;
        };
        let Some(value) = decode_op_value(reader, ctx, tctx, record_end_bit, op) else {
            break;
        };

        let dep_entries = parse_dep_entries(reader, ctx, tctx, record_end_bit);

        entries.push(Entry {
            key,
            value,
            dep_entries,
        });
    }

    entries
}

/// Parse dependency entries following a main entry
fn parse_dep_entries(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    tctx: &TableContext,
    record_end_bit: usize,
) -> Vec<DepEntry> {
    if tctx.dep_names.is_empty() {
        return Vec::new();
    }

    let mut dep_entries = Vec::new();

    while reader.position() + 2 <= record_end_bit {
        let Some(dep_op) = reader.read_bits(2) else {
            break;
        };
        if dep_op == 0 {
            break;
        }

        let Some(dep_key) = read_pair_vec_string(reader, ctx, tctx.pair_remap) else {
            break;
        };
        let dep_index = if tctx.dep_index_bits > 0 {
            reader.read_bits(tctx.dep_index_bits).unwrap_or(0)
        } else {
            0
        };

        let dep_table_name = tctx
            .dep_names
            .get(dep_index as usize)
            .cloned()
            .unwrap_or_default();

        let Some(dep_value) = decode_op_value(reader, ctx, tctx, record_end_bit, dep_op) else {
            break;
        };

        dep_entries.push(DepEntry {
            dep_table_name,
            dep_index,
            key: dep_key,
            value: dep_value,
        });
    }

    dep_entries
}

/// Parse all records from a table's record section
fn parse_records(
    reader: &mut BitReader,
    ctx: &DecodeContext,
    tctx: &TableContext,
) -> Vec<Record> {
    let mut records = Vec::new();

    loop {
        reader.align_byte();
        if !reader.has_bits(32) {
            break;
        }

        let record_start = reader.position();
        let Some(record_len_bytes) = reader.read_bits(32) else {
            break;
        };
        if record_len_bytes == 0 {
            if reader.has_bits(8) {
                reader.read_bits(8);
            }
            break;
        }

        let record_end_bit = (record_start + record_len_bytes as usize * 8) & !7;
        if record_end_bit > reader.total_bits() {
            break;
        }

        let tags = parse_tags(reader, ctx, tctx, record_end_bit);
        let entries = parse_entries(reader, ctx, tctx, record_end_bit);

        if reader.position() < record_end_bit {
            reader.seek(record_end_bit);
        }

        records.push(Record { tags, entries });
    }

    records
}

/// Input configuration for the decode loop
pub struct DecodeInput<'a> {
    pub data: &'a [u8],
    pub header_strings: &'a [String],
    pub value_strings: &'a [String],
    pub value_strings_declared: u32,
    pub value_kinds: &'a [String],
    pub value_kinds_declared: u32,
    pub key_strings: &'a [String],
    pub key_strings_declared: u32,
    pub row_flags: &'a [u32],
    pub binary_offset: usize,
}

/// Decode all table data from the binary section
pub fn decode_table_data(input: &DecodeInput) -> Option<Document> {
    if input.binary_offset >= input.data.len() {
        return None;
    }

    let binary_data = &input.data[input.binary_offset..];
    let mut reader = BitReader::new(binary_data);

    let ctx = DecodeContext {
        value_strings: input.value_strings,
        value_kinds: input.value_kinds,
        key_strings: input.key_strings,
        header_index_bits: bit_width(input.header_strings.len() as u32),
        value_index_bits: bit_width(input.value_strings_declared.max(1)),
        value_kind_bits: bit_width(input.value_kinds_declared.max(1)),
        key_index_bits: bit_width(input.key_strings_declared.max(1)),
        type_index_bits: bit_width(input.row_flags.len() as u32),
        row_flags: input.row_flags,
    };

    let table_id_bits = ctx.header_index_bits;
    let mut tables = HashMap::new();

    while reader.has_bits(table_id_bits as usize) {
        let table_id = reader.read_bits(table_id_bits)?;
        if table_id == 0 {
            break;
        }

        let table_name = input.header_strings.get(table_id as usize)?.clone();

        let (dep_names, dep_count) =
            read_table_deps(&mut reader, table_id_bits, input.header_strings);

        let remap_a = FixedWidthIntArray::read(&mut reader)?;
        let remap_b = FixedWidthIntArray::read(&mut reader)?;

        let tctx = TableContext {
            pair_remap: if remap_a.is_active() { Some(&remap_a) } else { None },
            value_remap: if remap_b.is_active() { Some(&remap_b) } else { None },
            dep_index_bits: if dep_count > 0 {
                bit_width(dep_count as u32)
            } else {
                0
            },
            dep_names,
        };

        reader.align_byte();

        let records = parse_records(&mut reader, &ctx, &tctx);

        tables.insert(
            table_name.clone(),
            Table {
                name: table_name,
                deps: tctx.dep_names,
                records,
            },
        );
    }

    Some(Document { tables })
}

/// Read dependency table IDs until a 0-terminator
fn read_table_deps(
    reader: &mut BitReader,
    table_id_bits: u8,
    header_strings: &[String],
) -> (Vec<String>, usize) {
    let mut dep_names = Vec::new();
    let mut count = 0;

    loop {
        if !reader.has_bits(table_id_bits as usize) {
            break;
        }
        let dep_id = match reader.read_bits(table_id_bits) {
            Some(0) | None => break,
            Some(id) => id,
        };
        count += 1;
        if let Some(name) = header_strings.get(dep_id as usize) {
            dep_names.push(name.clone());
        }
    }

    (dep_names, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_decode_context<'a>(
        key_strings: &'a [String],
        value_strings: &'a [String],
        value_kinds: &'a [String],
        row_flags: &'a [u32],
    ) -> DecodeContext<'a> {
        DecodeContext {
            value_strings,
            value_kinds,
            key_strings,
            header_index_bits: 1,
            value_index_bits: bit_width(value_strings.len().max(1) as u32),
            value_kind_bits: bit_width(value_kinds.len().max(1) as u32),
            key_index_bits: bit_width(key_strings.len().max(1) as u32),
            type_index_bits: bit_width(row_flags.len() as u32),
            row_flags,
        }
    }

    fn make_table_context() -> TableContext<'static> {
        TableContext {
            pair_remap: None,
            value_remap: None,
            dep_names: Vec::new(),
            dep_index_bits: 0,
        }
    }

    #[test]
    fn test_decode_empty_binary() {
        let data = vec![0u8; 4];
        let header_strings = vec!["test".to_string()];
        let row_flags = vec![0u32];

        let result = decode_table_data(&DecodeInput {
            data: &data,
            header_strings: &header_strings,
            value_strings: &[],
            value_strings_declared: 0,
            value_kinds: &[],
            value_kinds_declared: 0,
            key_strings: &[],
            key_strings_declared: 0,
            row_flags: &row_flags,
            binary_offset: 0,
        });

        let doc = result.unwrap();
        assert!(doc.tables.is_empty());
    }

    #[test]
    fn test_parse_tags_empty_z_terminator() {
        let data = [b'z'];
        let mut reader = BitReader::new(&data);
        let key_strings = vec!["none".to_string()];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);
        let tctx = make_table_context();

        let tags = parse_tags(&mut reader, &ctx, &tctx, data.len() * 8);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_tags_tag_a_key_name() {
        let key_strings: Vec<String> = vec![
            "none".to_string(),
            "test_key".to_string(),
        ];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);
        let tctx = make_table_context();

        // Tag 'a' (0x61), then key index 1 (1 bit since 2 keys), then 'z' (0x7A)
        // key_index_bits = bit_width(2) = 1
        // byte 0: 0x61 = 'a'
        // bit 8: key index = 1 (1 bit)
        // We need to pack: 8 bits of 'a', 1 bit of index=1, then 8 bits of 'z'
        // 0x61 = 0110_0001, then 1, then 0x7A = 0111_1010
        // byte 0: 0110_0001 = 0x61
        // byte 1: 1_0111_101 = 0xBD
        // byte 2: 0_0000000 = 0x00
        // Wait, BitReader reads MSB first. Let me reconsider.
        // bit_width(2) = 1, so key index 1 is read as 1 bit.
        // Actually BitReader reads bits MSB-first from bytes.
        // Byte 0 bits: [7,6,5,4,3,2,1,0] = 0,1,1,0,0,0,0,1 for 0x61
        // read_bits(8) reads bits 7..0 -> 0x61 = 'a' ✓
        // Then read_bits(1) reads bit 7 of byte 1
        // For index 1, we need that bit to be 1.
        // Then read_bits(8) for 'z' = 0x7A = 0111_1010
        // Byte 1: bit7=1, then bits6..0 = 0111101 (first 7 bits of 'z')
        // Byte 2: bit7=0 (last bit of 'z'), rest don't matter
        // Byte 1 = 1_0111101 = 0xBD
        // Byte 2 = 0_0000000 = 0x00
        let data = [0x61, 0xBD, 0x00];
        let mut reader = BitReader::new(&data);
        let tags = parse_tags(&mut reader, &ctx, &tctx, data.len() * 8);

        assert_eq!(tags.len(), 1);
        match &tags[0] {
            Tag::KeyName { pair } => assert_eq!(pair, "test_key"),
            other => panic!("Expected Tag::KeyName, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_tags_tag_b_u32() {
        let key_strings = vec!["none".to_string()];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);
        let tctx = make_table_context();

        // Tag 'b' (0x62) + 32 bits of value 42 in LE (BitReader is LSB-first) + tag 'z' (0x7A)
        let data = [0x62, 0x2A, 0x00, 0x00, 0x00, 0x7A];
        let mut reader = BitReader::new(&data);
        let tags = parse_tags(&mut reader, &ctx, &tctx, data.len() * 8);

        assert_eq!(tags.len(), 1);
        match &tags[0] {
            Tag::U32 { value } => assert_eq!(*value, 42),
            other => panic!("Expected Tag::U32, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_tags_tag_c_f32() {
        let key_strings = vec!["none".to_string()];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);
        let tctx = make_table_context();

        // Tag 'c' (0x63) + 32 bits of 1.0f (0x3F800000) in LE + tag 'z'
        // BitReader is LSB-first, so 0x3F800000 → bytes [0x00, 0x00, 0x80, 0x3F]
        let data = [0x63, 0x00, 0x00, 0x80, 0x3F, 0x7A];
        let mut reader = BitReader::new(&data);
        let tags = parse_tags(&mut reader, &ctx, &tctx, data.len() * 8);

        assert_eq!(tags.len(), 1);
        match &tags[0] {
            Tag::F32 {
                u32_value,
                f32_value,
            } => {
                assert_eq!(*u32_value, 0x3F800000);
                assert!((f32_value - 1.0).abs() < f32::EPSILON);
            }
            other => panic!("Expected Tag::F32, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_tags_multiple() {
        let key_strings = vec!["none".to_string()];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);
        let tctx = make_table_context();

        // Two 'b' tags then 'z', values in LE (BitReader is LSB-first)
        let data = [0x62, 0x01, 0x00, 0x00, 0x00, 0x62, 0x02, 0x00, 0x00, 0x00, 0x7A];
        let mut reader = BitReader::new(&data);
        let tags = parse_tags(&mut reader, &ctx, &tctx, data.len() * 8);

        assert_eq!(tags.len(), 2);
        match (&tags[0], &tags[1]) {
            (Tag::U32 { value: v1 }, Tag::U32 { value: v2 }) => {
                assert_eq!(*v1, 1);
                assert_eq!(*v2, 2);
            }
            other => panic!("Expected two Tag::U32, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_tags_unknown_tag_breaks() {
        let key_strings = vec!["none".to_string()];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);
        let tctx = make_table_context();

        // Unknown tag 0xFF should cause break, returning empty
        let data = [0xFF];
        let mut reader = BitReader::new(&data);
        let tags = parse_tags(&mut reader, &ctx, &tctx, data.len() * 8);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_read_packed_name_list_terminated_by_none() {
        // key_strings: [0]="none", [1]="foo", [2]="bar"
        // key_index_bits = bit_width(3) = 2
        // Read index 1 ("foo"), index 2 ("bar"), index 0 ("none" → terminator)
        let key_strings: Vec<String> = vec![
            "none".to_string(),
            "foo".to_string(),
            "bar".to_string(),
        ];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);

        // BitReader is LSB-first: 2-bit indices packed from bit 0 upward
        // index 1 = 0b01 at bits[0..2], index 2 = 0b10 at bits[2..4], index 0 = 0b00 at bits[4..6]
        // byte = (1 << 0) | (2 << 2) | (0 << 4) = 0x09
        let data = [0x09];
        let mut reader = BitReader::new(&data);
        let list = read_packed_name_list(&mut reader, &ctx, None).unwrap();

        assert_eq!(list, vec!["foo", "bar"]);
    }

    #[test]
    fn test_read_packed_name_list_empty() {
        let key_strings: Vec<String> = vec!["none".to_string(), "test".to_string()];
        let row_flags = vec![0u32];
        let ctx = make_decode_context(&key_strings, &[], &[], &row_flags);

        // 1-bit index: 0 (none → terminator immediately)
        let data = [0x00];
        let mut reader = BitReader::new(&data);
        let list = read_packed_name_list(&mut reader, &ctx, None).unwrap();

        assert!(list.is_empty());
    }

    #[test]
    fn test_read_table_deps() {
        let header_strings = vec![
            "unused".to_string(),
            "inv".to_string(),
            "inv_comp".to_string(),
            "firmware".to_string(),
        ];

        // table_id_bits = 2, BitReader is LSB-first
        // deps: 2 ("inv_comp"), 3 ("firmware"), 0 (terminator)
        // byte = (2 << 0) | (3 << 2) | (0 << 4) = 0x0E
        let data = [0x0E];
        let mut reader = BitReader::new(&data);

        let (dep_names, count) = read_table_deps(&mut reader, 2, &header_strings);

        assert_eq!(count, 2);
        assert_eq!(dep_names, vec!["inv_comp", "firmware"]);
    }

    #[test]
    fn test_read_table_deps_empty() {
        let header_strings = vec!["unused".to_string(), "inv".to_string()];

        // 1-bit index: 0 (terminator immediately)
        let data = [0x00];
        let mut reader = BitReader::new(&data);

        let (dep_names, count) = read_table_deps(&mut reader, 1, &header_strings);

        assert_eq!(count, 0);
        assert!(dep_names.is_empty());
    }

    #[test]
    fn test_wrap_with_self_key_nonempty() {
        let value = Value::Leaf("hello".to_string());
        let result = wrap_with_self_key("my_key".to_string(), value).unwrap();

        match result {
            Value::Map(map) => {
                assert_eq!(map.len(), 1);
                assert!(map.contains_key("my_key"));
            }
            other => panic!("Expected Map, got {:?}", other),
        }
    }

    #[test]
    fn test_wrap_with_self_key_empty() {
        let value = Value::Leaf("hello".to_string());
        let result = wrap_with_self_key(String::new(), value.clone()).unwrap();
        match result {
            Value::Leaf(s) => assert_eq!(s, "hello"),
            other => panic!("Expected Leaf, got {:?}", other),
        }
    }

    #[test]
    fn test_wrap_with_self_key_none() {
        let value = Value::Leaf("hello".to_string());
        let result = wrap_with_self_key("None".to_string(), value).unwrap();
        match result {
            Value::Leaf(s) => assert_eq!(s, "hello"),
            other => panic!("Expected Leaf (none key skipped), got {:?}", other),
        }
    }

    #[test]
    fn test_remap_index_no_remap() {
        let (bits, mapped) = remap_index(None, 5, 8);
        assert_eq!(bits, 8);
        assert_eq!(mapped, 5);
    }
}
