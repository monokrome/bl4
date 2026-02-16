//! NCS binary section parser
//!
//! Parses NCS binary payloads into structured documents with tables/records/entries.
//!
//! Pipeline: blob header → header strings → type code table → decode loop → Document

pub mod blob;
pub mod decode;
pub mod remap;
pub mod typecodes;

use crate::document::Document;
use blob::{extract_header_strings, BlobHeader};
use decode::{decode_table_data, DecodeInput};
use typecodes::parse_type_code_table;

/// Parse decompressed NCS data into a Document
///
/// This is the single entry point for NCS parsing. It:
/// 1. Parses the blob header (16 bytes)
/// 2. Extracts header strings (table/dep names)
/// 3. Parses the TypeCodeTable (type codes, bit matrix, 3 string blocks)
/// 4. Runs the decode loop to produce tables with records
pub fn parse(data: &[u8]) -> Option<Document> {
    let blob = BlobHeader::parse(data)?;
    let header_strings = extract_header_strings(data, &blob);

    if header_strings.is_empty() {
        return None;
    }

    let body_offset = blob.body_offset();
    if body_offset >= data.len() {
        return None;
    }

    let body = &data[body_offset..];
    let tct = parse_type_code_table(body)?;

    decode_table_data(&DecodeInput {
        data,
        header_strings: &header_strings,
        value_strings: &tct.value_strings,
        value_strings_declared: tct.value_strings_declared_count,
        value_kinds: &tct.value_kinds,
        value_kinds_declared: tct.value_kinds_declared_count,
        key_strings: &tct.key_strings,
        key_strings_declared: tct.key_strings_declared_count,
        row_flags: &tct.header.row_flags,
        binary_offset: body_offset + tct.data_offset,
    })
}

/// Extract dependency info from blob header strings
///
/// Returns (type_name, dep_names) where type_name is the first string
/// and dep_names are the remaining strings.
pub fn extract_deps(data: &[u8]) -> Option<(String, Vec<String>)> {
    let blob = BlobHeader::parse(data)?;
    let mut strings = extract_header_strings(data, &blob);

    if strings.is_empty() {
        return None;
    }

    let type_name = strings.remove(0);
    Some((type_name, strings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_too_short() {
        assert!(parse(&[]).is_none());
        assert!(parse(&[0; 10]).is_none());
    }

    #[test]
    fn test_extract_deps_too_short() {
        assert!(extract_deps(&[]).is_none());
    }
}
