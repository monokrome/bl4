//! Document parsing for NCS files
//!
//! Handles parsing of NCS documents based on format code (abjx, abij, abhj, etc.)

use crate::string_table::parse_string_table;
use crate::types::{Document, Header, Record, StringTable};

use super::entries::{parse_entries_format, parse_strings_as_records};
use super::header::parse_header;

/// Parse document based on format code
pub fn parse_document(data: &[u8]) -> Option<Document> {
    let header = parse_header(data)?;
    let string_table = parse_string_table(data, &header);

    let records = match header.format_code.as_str() {
        "abjx" => parse_abjx(data, &header, &string_table),
        "abij" => parse_abij(data, &header, &string_table),
        "abhj" => parse_abhj(data, &header, &string_table),
        "abpe" => parse_abpe(data, &header, &string_table),
        "abqr" => parse_abqr(data, &header, &string_table),
        _ => parse_generic(data, &header, &string_table),
    };

    Some(Document {
        type_name: header.type_name,
        format_code: header.format_code,
        records,
    })
}

/// Parse abjx format (most common)
/// Structure: entries with JSON-like fields, extended with dep_entries
fn parse_abjx(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header.field_count, &header.type_name, strings, true)
}

/// Parse abij format
/// Structure: indexed entries with JSON-like fields
fn parse_abij(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header.field_count, &header.type_name, strings, false)
}

/// Parse abhj format
/// Structure: hash-indexed entries with JSON-like fields
fn parse_abhj(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header.field_count, &header.type_name, strings, false)
}

/// Parse abpe format
/// Structure: property-based entries (used by audio_event)
fn parse_abpe(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header.field_count, &header.type_name, strings, false)
}

/// Parse abqr format
/// Structure: quiet/reference format (used by DialogQuietTime)
fn parse_abqr(_data: &[u8], _header: &Header, strings: &StringTable) -> Vec<Record> {
    // abqr has offset tables at the start - different structure
    // For now, extract what we can from strings
    parse_strings_as_records(strings)
}

/// Generic fallback parser
fn parse_generic(_data: &[u8], _header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_strings_as_records(strings)
}
