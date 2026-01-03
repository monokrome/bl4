//! Binary section parsing for NCS files
//!
//! The binary section contains bit-packed string indices and structured metadata.

use crate::bit_reader::{bit_width, BitReader};
use crate::types::{BinaryParseResult, EntryGroup, FieldInfo, StringTable};

/// Parse the binary section of an NCS file
///
/// The binary section has two main parts:
/// 1. Bit-packed string indices (first ~32 bytes, variable)
/// 2. Structured metadata section (byte values separated by 0x28 or 0x20)
///
/// The structured metadata creates entry groups that correspond to entries
/// in the string table.
pub fn parse_binary_section(
    data: &[u8],
    binary_offset: usize,
    strings: &StringTable,
) -> Option<BinaryParseResult> {
    if binary_offset >= data.len() {
        return None;
    }

    let binary_data = &data[binary_offset..];

    // Calculate bit width for string table lookup
    let string_bits = bit_width(strings.len() as u32);

    // Find the structured metadata section by looking for byte values
    // in the 0x08-0x28 range followed by 0x28/0x20 separators
    let structured_start = find_structured_section_start(binary_data);

    // Part 1: Read bit-packed indices from the first section
    let bit_section = &binary_data[..structured_start.min(binary_data.len())];
    let bit_indices = read_bit_packed_indices(bit_section, string_bits);

    // Get table_id (first bit-packed value)
    let table_id = bit_indices.first().copied().unwrap_or(0);

    // Part 2: Parse structured metadata section
    let (entry_groups, tail_start) = if structured_start < binary_data.len() {
        parse_structured_section(&binary_data[structured_start..])
    } else {
        (Vec::new(), 0)
    };

    // Part 3: Extract tail data
    let tail_offset = structured_start + tail_start;
    let tail_data = if tail_offset < binary_data.len() {
        binary_data[tail_offset..].to_vec()
    } else {
        Vec::new()
    };

    Some(BinaryParseResult {
        table_id,
        bit_indices,
        entry_groups,
        tail_data,
    })
}

/// Find where the structured metadata section starts
///
/// The bit-packed section typically ends when we see a pattern of
/// byte values in the 0x08-0x30 range (field metadata), followed
/// by a separator (0x28 or 0x20) and terminator (0x00 0x00).
///
/// Two format variations:
/// 1. Separator format: values 0x08-0x40 with 0x28/0x20 separators
/// 2. Compact format: 0x80 0x80 header followed by fixed-width records
fn find_structured_section_start(data: &[u8]) -> usize {
    // First, check for 0x28 separator - if present, use separator format
    let has_separator = data.iter().any(|&b| b == 0x28);

    if has_separator {
        // Use separator format detection (skip compact format check)
    } else {
        // Check for compact format (0x80 0x80 header)
        // This pattern appears when there are no 0x28 separators
        for i in 16..data.len().saturating_sub(4) {
            if data[i] == 0x80 && data[i + 1] == 0x80 {
                // Verify there's a 00 00 terminator ahead
                let has_terminator = data[i + 2..]
                    .windows(2)
                    .take(48)
                    .any(|w| w == [0x00, 0x00]);

                if has_terminator {
                    return i;
                }
            }
        }
    }

    // Strategy: Find the first 0x28 separator and walk back to find
    // the start of the structured section.
    //
    // The structured section has:
    // - Values mostly in 0x08-0x30 range
    // - 0x28 or 0x20 separators between groups
    // - 0x00 0x00 terminator

    // First, find the position of the first 0x28 separator
    let first_sep = data.iter().position(|&b| b == 0x28);

    if let Some(sep_pos) = first_sep {
        // Walk backwards from separator to find start of first group
        // The structured section values are typically >= 0x08 and <= 0x40
        let mut start = sep_pos;

        while start > 0 {
            let prev = data[start - 1];
            // Valid structured values are in range 0x08-0x40
            // Skip high bytes like 0x80, 0xff which are bit-packed data
            if prev >= 0x08 && prev <= 0x40 {
                start -= 1;
            } else {
                break;
            }
        }

        // Verify this is a reasonable start
        if start < sep_pos && start >= 16 {
            return start;
        }
    }

    // Alternative: look for a clean transition pattern
    // Bit-packed data often has high bytes (0x80+), structured has low bytes
    for i in 24..data.len().saturating_sub(16) {
        // Check for transition: high byte(s) followed by low byte run
        let has_high_before = i > 0 && (data[i - 1] >= 0x80 || data[i - 1] < 0x08);
        let has_low_run = data[i..].iter().take(8).all(|&b| b <= 0x40);
        let has_separator = data[i..].iter().take(16).any(|&b| b == 0x28 || b == 0x20);
        let has_terminator = data[i..]
            .windows(2)
            .take(48)
            .any(|w| w == [0x00, 0x00]);

        if has_high_before && has_low_run && has_separator && has_terminator {
            return i;
        }
    }

    // If no structured section found, assume it's all bit-packed
    data.len()
}

/// Read bit-packed string indices from the first section
fn read_bit_packed_indices(data: &[u8], bit_width: u8) -> Vec<u32> {
    let mut indices = Vec::new();
    let mut reader = BitReader::new(data);

    // Read indices until we run out of data
    while reader.has_bits(bit_width as usize) {
        if let Some(idx) = reader.read_bits(bit_width) {
            indices.push(idx);
        } else {
            break;
        }
    }

    indices
}

/// Parse the structured metadata section into entry groups
///
/// Returns (entry_groups, tail_start_offset)
///
/// Two formats are supported:
/// 1. Separator format: Groups separated by 0x28 or 0x20, ending with 00 00
/// 2. Compact format: 0x80 0x80 header followed by fixed-width records
fn parse_structured_section(data: &[u8]) -> (Vec<EntryGroup>, usize) {
    // Check for compact format (0x80 0x80 header)
    if data.len() >= 4 && data[0] == 0x80 && data[1] == 0x80 {
        return parse_compact_structured_section(data);
    }

    // Standard separator format
    let mut groups = Vec::new();
    let mut current_values = Vec::new();
    let mut tail_start = data.len();

    for (i, &byte) in data.iter().enumerate() {
        // Check for terminator (00 00)
        if byte == 0x00 {
            if i + 1 < data.len() && data[i + 1] == 0x00 {
                // Save current group if any
                if !current_values.is_empty() {
                    groups.push(create_entry_group(current_values.clone()));
                }
                tail_start = i + 2; // Skip past 00 00
                break;
            }
            continue;
        }

        // Check for separator (0x28 or 0x20)
        if byte == 0x28 || byte == 0x20 {
            if !current_values.is_empty() {
                groups.push(create_entry_group(current_values.clone()));
                current_values.clear();
            }
            continue;
        }

        // Add byte to current group
        current_values.push(byte);
    }

    // Handle any remaining values
    if !current_values.is_empty() {
        groups.push(create_entry_group(current_values));
    }

    (groups, tail_start)
}

/// Parse compact structured section (0x80 0x80 header format)
///
/// This format is used by files like rarity that don't have separators.
/// Structure: 0x80 0x80 [byte pairs for each entry] 00 00
fn parse_compact_structured_section(data: &[u8]) -> (Vec<EntryGroup>, usize) {
    let mut groups = Vec::new();

    // Skip the 0x80 0x80 header
    let start = 2;
    let mut tail_start = data.len();

    // Find terminator (00 00)
    let end = data[start..]
        .windows(2)
        .position(|w| w == [0x00, 0x00])
        .map(|pos| start + pos)
        .unwrap_or(data.len());

    if end > start {
        let payload = &data[start..end];

        // Try to determine record width
        // Common patterns: 2 bytes per entry
        let record_width = 2;

        for chunk in payload.chunks(record_width) {
            groups.push(create_entry_group(chunk.to_vec()));
        }

        tail_start = end + 2; // Skip past 00 00
    }

    (groups, tail_start)
}

/// Create an entry group from raw byte values
fn create_entry_group(values: Vec<u8>) -> EntryGroup {
    // Interpret values as field metadata
    // Each value might represent bit offset, bit width, or position
    let mut field_info = Vec::new();
    let mut bit_offset = 0u32;

    for &val in &values {
        // Hypothesis: values represent cumulative bit offsets or widths
        field_info.push(FieldInfo {
            bit_offset,
            bit_width: val,
            string_index: None,
        });
        bit_offset += val as u32;
    }

    EntryGroup { values, field_info }
}

/// Debug function to dump binary section info
pub fn debug_binary_section(data: &[u8], binary_offset: usize) -> String {
    let mut output = String::new();
    use std::fmt::Write;

    if binary_offset >= data.len() {
        return "Binary offset out of bounds".to_string();
    }

    let binary_data = &data[binary_offset..];
    let _ = writeln!(output, "Binary section at 0x{:x}, {} bytes", binary_offset, binary_data.len());

    // Show first 64 bytes as hex
    let preview_len = binary_data.len().min(64);
    let _ = writeln!(output, "First {} bytes:", preview_len);
    for (i, chunk) in binary_data[..preview_len].chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
        let _ = writeln!(output, "  {:04x}: {}", i * 16, hex.join(" "));
    }

    // Try to read as bits
    let mut reader = BitReader::new(binary_data);
    let _ = writeln!(output, "\nBit reading test:");

    // Try reading first few values
    if let Some(v) = reader.read_bits(8) {
        let _ = writeln!(output, "  First 8 bits: {} (0x{:02x})", v, v);
    }
    if let Some(v) = reader.read_bits(8) {
        let _ = writeln!(output, "  Next 8 bits: {} (0x{:02x})", v, v);
    }
    if let Some(v) = reader.read_bits(16) {
        let _ = writeln!(output, "  Next 16 bits: {} (0x{:04x})", v, v);
    }

    output
}
