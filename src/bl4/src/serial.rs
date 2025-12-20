//! Item serial number decoding for Borderlands 4
//!
//! Item serials use a custom Base85 encoding with bit-packed data.
//!
//! Format:
//! 1. Serials start with `@U` prefix
//! 2. Encoded with custom Base85 alphabet
//! 3. Decoded bytes have mirrored bits
//! 4. Data is a variable-length bitstream with tokens

use crate::parts::{
    item_type_name, level_from_code, manufacturer_name, serial_format,
    weapon_info_from_first_varint,
};

/// Custom Base85 alphabet used by Borderlands 4
const BL4_BASE85_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~";

/// Maximum reasonable part index. Part indices above this threshold indicate
/// we're parsing garbage data (likely over-reading past the end of valid tokens).
/// Based on analysis: most parts have indices < 100, max observed valid is ~300.
const MAX_REASONABLE_PART_INDEX: u64 = 1000;

/// Errors that can occur during serial decoding
#[derive(Debug, thiserror::Error)]
pub enum SerialError {
    #[error("Invalid Base85 encoding: {0}")]
    InvalidEncoding(String),

    #[error("Serial too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },

    #[error("Unknown item type: {0}")]
    UnknownItemType(char),
}

/// Bitstream reader for parsing variable-length tokens
struct BitReader {
    bytes: Vec<u8>,
    bit_offset: usize,
}

/// Bitstream writer for encoding variable-length tokens
struct BitWriter {
    bytes: Vec<u8>,
    bit_offset: usize,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_offset: 0,
        }
    }

    /// Write N bits from a u64 value (MSB-first)
    fn write_bits(&mut self, value: u64, count: usize) {
        for i in (0..count).rev() {
            let bit = ((value >> i) & 1) as u8;
            let byte_idx = self.bit_offset / 8;
            let bit_idx = 7 - (self.bit_offset % 8); // Write from MSB (bit 7) down to LSB (bit 0)

            // Extend bytes vector if needed
            while byte_idx >= self.bytes.len() {
                self.bytes.push(0);
            }

            if bit == 1 {
                self.bytes[byte_idx] |= 1 << bit_idx;
            }
            self.bit_offset += 1;
        }
    }

    /// Write a VARINT (4-bit nibbles with continuation bits)
    fn write_varint(&mut self, value: u64) {
        let mut remaining = value;

        loop {
            let nibble = remaining & 0xF;
            remaining >>= 4;

            self.write_bits(nibble, 4);

            if remaining == 0 {
                self.write_bits(0, 1); // Continuation = 0 (stop)
                break;
            } else {
                self.write_bits(1, 1); // Continuation = 1 (more)
            }
        }
    }

    /// Write a VARBIT (5-bit length prefix + variable data)
    fn write_varbit(&mut self, value: u64) {
        if value == 0 {
            self.write_bits(0, 5); // Length 0 means value 0
            return;
        }

        // Calculate number of bits needed
        let bits_needed = 64 - value.leading_zeros() as usize;
        self.write_bits(bits_needed as u64, 5);
        self.write_bits(value, bits_needed);
    }

    /// Get the final bytes (padded to byte boundary)
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl BitReader {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    /// Read N bits as a u64 value (MSB-first)
    /// Bits are read from the stream and assembled with first bit = MSB
    fn read_bits(&mut self, count: usize) -> Option<u64> {
        if count > 64 {
            return None;
        }

        let mut result = 0u64;
        for _ in 0..count {
            let byte_idx = self.bit_offset / 8;
            let bit_idx = 7 - (self.bit_offset % 8); // Read from MSB (bit 7) down to LSB (bit 0)

            if byte_idx >= self.bytes.len() {
                return None;
            }

            let bit = (self.bytes[byte_idx] >> bit_idx) & 1;
            result = (result << 1) | (bit as u64);
            self.bit_offset += 1;
        }

        Some(result)
    }

    /// Read a VARINT (4-bit nibbles with continuation bits)
    /// Format: [4-bit value][1-bit continuation]... Values assembled LSB-first.
    /// Continuation bit 1 = more nibbles follow, 0 = stop.
    fn read_varint(&mut self) -> Option<u64> {
        let mut result = 0u64;
        let mut shift = 0;

        // Max 4 nibbles (16 bits total)
        for _ in 0..4 {
            let nibble = self.read_bits(4)?;
            result |= nibble << shift;
            shift += 4;

            // Read continuation bit (1 = continue, 0 = stop)
            let cont = self.read_bits(1)?;
            if cont == 0 {
                return Some(result);
            }
        }

        Some(result)
    }

    /// Read a VARBIT (5-bit length prefix + variable data)
    /// Format: [5-bit length][N-bit value]. Length 0 means value is 0.
    fn read_varbit(&mut self) -> Option<u64> {
        let length = self.read_bits(5)? as usize;
        self.read_bits(length)
    }

    #[allow(dead_code)]
    fn current_bit_offset(&self) -> usize {
        self.bit_offset
    }

    /// Returns the number of bits remaining in the stream
    fn remaining_bits(&self) -> usize {
        let total_bits = self.bytes.len() * 8;
        total_bits.saturating_sub(self.bit_offset)
    }
}

/// Token types in the bitstream
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Separator,                             // 00
    SoftSeparator,                         // 01
    VarInt(u64),                           // 100 + varint data
    VarBit(u64),                           // 110 + varbit data
    Part { index: u64, values: Vec<u64> }, // 101 + part data
    String(String),                        // 111 + length + ascii
}

/// Decoded item serial information
#[derive(Debug, Clone)]
pub struct ItemSerial {
    /// Original ASCII85-encoded serial
    pub original: String,

    /// Raw decoded bytes
    pub raw_bytes: Vec<u8>,

    /// Item type character (r=weapon, e=equipment, etc.)
    pub item_type: char,

    /// Parsed tokens from bitstream
    pub tokens: Vec<Token>,

    /// Decoded fields (extracted from tokens)
    /// For VarInt-first format: Combined manufacturer + weapon type ID
    pub manufacturer: Option<u64>,
    /// Item level (fourth VarInt for VarInt-first format)
    pub level: Option<u64>,
    /// Random seed for stat rolls (second VarInt after first separator)
    pub seed: Option<u64>,
}

/// Decode Base85 with custom BL4 alphabet
fn decode_base85(input: &str) -> Result<Vec<u8>, SerialError> {
    // Build reverse lookup table
    let mut lookup = [0u8; 256];
    for (i, &ch) in BL4_BASE85_ALPHABET.iter().enumerate() {
        lookup[ch as usize] = i as u8;
    }

    let mut result = Vec::new();
    let chars: Vec<char> = input.chars().collect();

    // Process in chunks of 5 characters -> 4 bytes
    for chunk in chars.chunks(5) {
        let mut value: u64 = 0;

        for &ch in chunk.iter() {
            let byte_val = lookup[ch as usize] as u64;
            value = value * 85 + byte_val;
        }

        // Convert to big-endian bytes (most significant first)
        let num_bytes = if chunk.len() == 5 { 4 } else { chunk.len() - 1 };
        for i in (0..num_bytes).rev() {
            result.push(((value >> (i * 8)) & 0xFF) as u8);
        }
    }

    Ok(result)
}

/// Mirror bits in a byte (reverse bit order)
/// Example: 0b10000111 -> 0b11100001
#[inline]
fn mirror_byte(byte: u8) -> u8 {
    byte.reverse_bits()
}

/// Encode bytes to Base85 with custom BL4 alphabet
fn encode_base85(bytes: &[u8]) -> String {
    let mut result = String::new();

    // Process in chunks of 4 bytes -> 5 characters
    for chunk in bytes.chunks(4) {
        // Build value from bytes (big-endian)
        let mut value: u64 = 0;
        for &byte in chunk {
            value = (value << 8) | (byte as u64);
        }

        // Pad if chunk is smaller than 4 bytes
        if chunk.len() < 4 {
            value <<= (4 - chunk.len()) * 8;
        }

        // Convert to base85 (5 characters for 4 bytes, less for smaller chunks)
        let num_chars = if chunk.len() == 4 { 5 } else { chunk.len() + 1 };
        let mut chars = vec![0u8; num_chars];

        for i in (0..num_chars).rev() {
            chars[i] = BL4_BASE85_ALPHABET[(value % 85) as usize];
            value /= 85;
        }

        for &ch in &chars {
            result.push(ch as char);
        }
    }

    result
}

/// Encode tokens back to bitstream bytes
fn encode_tokens(tokens: &[Token]) -> Vec<u8> {
    let mut writer = BitWriter::new();

    // Write magic header (7 bits = 0010000)
    writer.write_bits(0b0010000, 7);

    for token in tokens {
        match token {
            Token::Separator => {
                writer.write_bits(0b00, 2);
            }
            Token::SoftSeparator => {
                writer.write_bits(0b01, 2);
            }
            Token::VarInt(v) => {
                writer.write_bits(0b100, 3);
                writer.write_varint(*v);
            }
            Token::VarBit(v) => {
                writer.write_bits(0b110, 3);
                writer.write_varbit(*v);
            }
            Token::Part { index, values } => {
                writer.write_bits(0b101, 3);
                writer.write_varint(*index);

                if values.is_empty() {
                    // SUBTYPE_NONE: type=0, subtype=10
                    writer.write_bits(0, 1);
                    writer.write_bits(0b10, 2);
                } else if values.len() == 1 {
                    // SUBTYPE_INT: type=1, value, 000 terminator
                    writer.write_bits(1, 1);
                    writer.write_varint(values[0]);
                    writer.write_bits(0b000, 3);
                } else {
                    // SUBTYPE_LIST: type=0, subtype=01, values, 00 terminator
                    writer.write_bits(0, 1);
                    writer.write_bits(0b01, 2);
                    for v in values {
                        // For simplicity, encode as VarInt
                        writer.write_bits(0b100, 3);
                        writer.write_varint(*v);
                    }
                    writer.write_bits(0b00, 2); // Separator to terminate
                }
            }
            Token::String(s) => {
                writer.write_bits(0b111, 3);
                writer.write_varint(s.len() as u64);
                for ch in s.bytes() {
                    writer.write_bits(ch as u64, 7);
                }
            }
        }
    }

    writer.finish()
}

/// Parse tokens from bitstream
fn parse_tokens(reader: &mut BitReader) -> Vec<Token> {
    parse_tokens_impl(reader, false)
}

/// Parse tokens with optional debug output
fn parse_tokens_impl(reader: &mut BitReader, debug: bool) -> Vec<Token> {
    let mut tokens = Vec::new();

    // Verify magic header (7 bits = 0010000)
    if let Some(magic) = reader.read_bits(7) {
        if magic != 0b0010000 {
            if debug {
                eprintln!("Warning: Invalid magic header: {:07b}", magic);
            }
            return tokens;
        }
        if debug {
            eprintln!("[bit {:3}] Magic header OK", reader.bit_offset);
        }
    } else {
        return tokens;
    }

    // Parse tokens until terminator (00) or end of data
    let mut iteration = 0;
    while iteration < 100 {
        // Safety limit
        iteration += 1;
        let bit_pos = reader.bit_offset;

        let prefix2 = match reader.read_bits(2) {
            Some(p) => p,
            None => break,
        };

        if debug {
            eprintln!("[bit {:3}] Prefix: {:02b}", bit_pos, prefix2);
        }

        match prefix2 {
            0b00 => {
                if debug {
                    eprintln!(
                        "         -> Separator (remaining bits: {})",
                        reader.remaining_bits()
                    );
                }
                tokens.push(Token::Separator);
                // If we have very few bits left after a separator, stop parsing.
                // This prevents interpreting trailing padding/garbage as tokens.
                // Minimum meaningful token is 3 bits (prefix) + 5 bits (min varint) = 8 bits
                if reader.remaining_bits() < 8 {
                    if debug {
                        eprintln!("         -> Insufficient bits remaining, stopping parse");
                    }
                    break;
                }
            }
            0b01 => {
                if debug {
                    eprintln!("         -> SoftSeparator");
                }
                tokens.push(Token::SoftSeparator);
            }
            0b10 | 0b11 => {
                // Need one more bit to distinguish
                let bit3 = match reader.read_bits(1) {
                    Some(b) => b,
                    None => break,
                };

                let prefix3 = (prefix2 << 1) | bit3;

                if debug {
                    eprintln!("         -> 3-bit prefix: {:03b}", prefix3);
                }

                match prefix3 {
                    0b100 => {
                        // VARINT
                        if let Some(val) = reader.read_varint() {
                            if debug {
                                eprintln!("         -> VarInt({})", val);
                            }
                            tokens.push(Token::VarInt(val));
                        }
                    }
                    0b110 => {
                        // VARBIT
                        if let Some(val) = reader.read_varbit() {
                            if debug {
                                eprintln!("         -> VarBit({})", val);
                            }
                            tokens.push(Token::VarBit(val));
                        }
                    }
                    0b101 => {
                        // Part structure:
                        // [VARINT index][1-bit type flag]
                        //   Type 1: [VARINT value][000 terminator]
                        //   Type 0: [2-bit subtype]
                        //     10 = no data
                        //     01 = value list until 00 terminator
                        if let Some(index) = reader.read_varint() {
                            // Validate part index is reasonable. If it's too large,
                            // we're likely parsing garbage data past the end of valid tokens.
                            if index > MAX_REASONABLE_PART_INDEX {
                                if debug {
                                    eprintln!("         -> Part index {} exceeds max ({}), stopping parse",
                                              index, MAX_REASONABLE_PART_INDEX);
                                }
                                break;
                            }

                            let mut values = Vec::new();

                            if let Some(type_flag) = reader.read_bits(1) {
                                if type_flag == 1 {
                                    // SUBTYPE_INT: single VARINT value + 000 terminator
                                    if let Some(val) = reader.read_varint() {
                                        values.push(val);
                                    }
                                    // Read 000 terminator (3 bits)
                                    let _ = reader.read_bits(3);
                                } else {
                                    // Type 0: read 2-bit subtype
                                    if let Some(subtype) = reader.read_bits(2) {
                                        match subtype {
                                            0b10 => {
                                                // SUBTYPE_NONE: no additional data
                                            }
                                            0b01 => {
                                                // SUBTYPE_LIST: read values until 00 separator
                                                // Values can be VARINT or VARBIT
                                                loop {
                                                    // Peek at next 2 bits to check for terminator
                                                    let start_pos = reader.bit_offset;
                                                    if let Some(peek) = reader.read_bits(2) {
                                                        if peek == 0b00 {
                                                            // Hard separator - end of list
                                                            break;
                                                        }
                                                        // Not a terminator, need to parse a value
                                                        // Read 1 more bit to get 3-bit prefix
                                                        if let Some(bit3) = reader.read_bits(1) {
                                                            let prefix3 = (peek << 1) | bit3;
                                                            match prefix3 {
                                                                0b100 => {
                                                                    if let Some(v) =
                                                                        reader.read_varint()
                                                                    {
                                                                        values.push(v);
                                                                    }
                                                                }
                                                                0b110 => {
                                                                    if let Some(v) =
                                                                        reader.read_varbit()
                                                                    {
                                                                        values.push(v);
                                                                    }
                                                                }
                                                                _ => {
                                                                    // Unknown prefix, rewind and break
                                                                    reader.bit_offset = start_pos;
                                                                    break;
                                                                }
                                                            }
                                                        } else {
                                                            break;
                                                        }
                                                    } else {
                                                        break;
                                                    }
                                                }
                                            }
                                            _ => {
                                                // Unknown subtype
                                            }
                                        }
                                    }
                                }
                            }

                            if debug {
                                eprintln!(
                                    "         -> Part {{ index: {}, values: {:?} }}",
                                    index, values
                                );
                            }
                            tokens.push(Token::Part { index, values });
                        }
                    }
                    0b111 => {
                        // String: VARINT length + 7-bit ASCII chars
                        if let Some(length) = reader.read_varint() {
                            if debug {
                                eprintln!(
                                    "         -> String length: {} (would need {} bits)",
                                    length,
                                    length * 7
                                );
                            }
                            // Sanity check - don't read more than 128 chars
                            if length > 128 {
                                if debug {
                                    eprintln!("         -> String too long, skipping");
                                }
                                continue;
                            }
                            let mut chars = Vec::new();
                            for _ in 0..length {
                                // 7-bit ASCII (LSB-first)
                                if let Some(ch) = reader.read_bits(7) {
                                    chars.push(ch as u8);
                                }
                            }
                            if let Ok(s) = String::from_utf8(chars.clone()) {
                                if debug {
                                    eprintln!("         -> String({:?})", s);
                                }
                                tokens.push(Token::String(s));
                            } else {
                                if debug {
                                    eprintln!("         -> String (binary): {:?}", chars);
                                }
                                // Store as lossy string anyway
                                tokens.push(Token::String(
                                    String::from_utf8_lossy(&chars).to_string(),
                                ));
                            }
                        }
                    }
                    _ => break,
                }
            }
            _ => break,
        }
    }

    tokens
}

/// Debug version of parse_tokens that prints what it sees
/// Note: expects already-mirrored bytes (as stored in ItemSerial.raw_bytes)
pub fn parse_tokens_debug(bytes: &[u8]) -> Vec<Token> {
    let mut reader = BitReader::new(bytes.to_vec());
    parse_tokens_impl(&mut reader, true)
}

impl ItemSerial {
    /// Decode a Borderlands 4 item serial
    ///
    /// Format: @Ug<type><base85_data>
    /// Example: @Ugr$ZCm/&tH!t{KgK/Shxu>k
    pub fn decode(serial: &str) -> Result<Self, SerialError> {
        // Check for @Ug prefix
        if !serial.starts_with("@Ug") {
            return Err(SerialError::InvalidEncoding(
                "Serial must start with @Ug".to_string(),
            ));
        }

        // Extract item type (character after @Ug)
        let item_type = serial.chars().nth(3).ok_or_else(|| {
            SerialError::InvalidEncoding("Serial too short - no item type".to_string())
        })?;

        // Strip @Ug prefix, keeping the item type and data
        let encoded_data = &serial[2..]; // Keep everything after @U

        // Decode Base85
        let decoded = decode_base85(encoded_data)?;

        // Mirror all bits
        let raw_bytes: Vec<u8> = decoded.iter().map(|&b| mirror_byte(b)).collect();

        if raw_bytes.len() < 4 {
            return Err(SerialError::TooShort {
                expected: 4,
                actual: raw_bytes.len(),
            });
        }

        // Parse the bitstream
        let mut reader = BitReader::new(raw_bytes.clone());
        let tokens = parse_tokens(&mut reader);

        // Extract common fields from tokens based on format
        let mut manufacturer = None;
        let mut level = None;
        let mut seed = None;

        // Determine format from first token (more reliable than type character)
        let is_varbit_first = matches!(tokens.first(), Some(Token::VarBit(_)));

        if is_varbit_first {
            // Equipment format: VBIT(category) SEP VBIT(level) ...
            // Level is the VarBit after the first separator
            let mut seen_first_sep = false;
            for token in &tokens {
                match token {
                    Token::Separator if !seen_first_sep => {
                        seen_first_sep = true;
                    }
                    Token::VarBit(v) if seen_first_sep && level.is_none() => {
                        level = level_from_code(*v).map(|l| l as u64);
                        break;
                    }
                    _ => {}
                }
            }
        } else {
            // Weapon format: VINT(mfg) SOFT VINT(0) SOFT VINT(8) SOFT VINT(level) SEP ...
            // Collect VarInts before first separator for header analysis
            let mut header_varints: Vec<u64> = Vec::new();
            let mut after_first_sep: Vec<u64> = Vec::new();
            let mut seen_separator = false;

            for token in &tokens {
                match token {
                    Token::VarInt(v) => {
                        if seen_separator {
                            after_first_sep.push(*v);
                        } else {
                            header_varints.push(*v);
                        }
                    }
                    Token::Separator => {
                        seen_separator = true;
                    }
                    _ => {}
                }
            }

            // Extract fields based on serial format
            if let Some(fmt) = serial_format(item_type) {
                // Only extract manufacturer ID for VarInt-first weapon formats
                if fmt.has_weapon_info && !header_varints.is_empty() {
                    manufacturer = Some(header_varints[0]);
                }
                if fmt.extract_level && header_varints.len() >= 4 {
                    level = level_from_code(header_varints[3]).map(|l| l as u64);
                }
                if fmt.has_weapon_info && after_first_sep.len() >= 2 {
                    seed = Some(after_first_sep[1]);
                }
            }
        }

        Ok(ItemSerial {
            original: serial.to_string(),
            raw_bytes,
            item_type,
            tokens,
            manufacturer,
            level,
            seed,
        })
    }

    /// Encode this item serial back to a Base85 string
    ///
    /// This encodes the current tokens back to a serial string.
    /// Useful for modifying an item and getting the new serial.
    pub fn encode(&self) -> String {
        // For now, we can't perfectly re-encode because we don't preserve
        // the original bytes that encode the item type. Instead, re-encode
        // from the original raw_bytes which preserves all information.
        //
        // TODO: Implement proper token-to-bytes encoding that includes item type
        let mirrored: Vec<u8> = self.raw_bytes.iter().map(|&b| mirror_byte(b)).collect();
        let encoded = encode_base85(&mirrored);
        format!("@U{}", encoded)
    }

    /// Encode with modified tokens (experimental)
    /// This attempts to encode tokens back to bytes, but may not preserve
    /// all original data like item type encoding.
    pub fn encode_from_tokens(&self) -> String {
        // Encode tokens to bytes
        let bytes = encode_tokens(&self.tokens);

        // Mirror all bits (reverse of decode)
        let mirrored: Vec<u8> = bytes.iter().map(|&b| mirror_byte(b)).collect();

        // Encode to Base85
        let encoded = encode_base85(&mirrored);

        // Build final serial with prefix (just @U since g and item_type are in the bytes)
        format!("@U{}", encoded)
    }

    /// Create a new ItemSerial with modified tokens
    pub fn with_tokens(&self, tokens: Vec<Token>) -> Self {
        ItemSerial {
            original: self.original.clone(),
            raw_bytes: self.raw_bytes.clone(), // Will be stale but that's OK
            item_type: self.item_type,
            tokens,
            manufacturer: self.manufacturer,
            level: self.level,
            seed: self.seed,
        }
    }

    /// Display hex dump of raw bytes
    pub fn hex_dump(&self) -> String {
        hex::encode(&self.raw_bytes)
    }

    /// Format tokens as human-readable string
    /// Example: `134, 0, 8, 196| 4, 2379|| {8} {4} {2} {8:3} {34}`
    pub fn format_tokens(&self) -> String {
        let mut output = String::new();

        for token in &self.tokens {
            match token {
                Token::Separator => output.push('|'),
                Token::SoftSeparator => output.push_str(", "),
                Token::VarInt(v) => output.push_str(&format!("{}", v)),
                Token::VarBit(v) => output.push_str(&format!("{}", v)),
                Token::Part { index, values } => {
                    if values.is_empty() {
                        output.push_str(&format!("{{{}}}", index));
                    } else if values.len() == 1 {
                        output.push_str(&format!("{{{}:{}}}", index, values[0]));
                    } else {
                        let vals: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                        output.push_str(&format!("{{{}:[{}]}}", index, vals.join(" ")));
                    }
                }
                Token::String(s) => {
                    if s.is_empty() {
                        output.push_str("\"\"");
                    } else {
                        output.push_str(&format!("{:?}", s));
                    }
                }
            }
            output.push(' ');
        }

        output.trim().to_string()
    }

    /// Get item type description
    pub fn item_type_description(&self) -> &'static str {
        item_type_name(self.item_type)
    }

    /// Get manufacturer name if known
    pub fn manufacturer_name(&self) -> Option<&'static str> {
        self.manufacturer.and_then(manufacturer_name)
    }

    /// Get weapon info (manufacturer, weapon type) for VarInt-first format serials
    ///
    /// Returns None for VarBit-first formats or if the ID is unknown.
    pub fn weapon_info(&self) -> Option<(&'static str, &'static str)> {
        let fmt = serial_format(self.item_type)?;
        if fmt.has_weapon_info {
            self.manufacturer.and_then(weapon_info_from_first_varint)
        } else {
            None
        }
    }

    /// Extract Part Group ID (category) from the serial
    ///
    /// Uses the format's category_divisor to extract category from first VarBit.
    /// Returns None if this format doesn't use VarBit categories.
    pub fn part_group_id(&self) -> Option<i64> {
        let fmt = serial_format(self.item_type)?;
        let first_varbit = self.tokens.iter().find_map(|t| {
            if let Token::VarBit(v) = t {
                Some(*v)
            } else {
                None
            }
        })?;
        fmt.extract_category(first_varbit)
    }

    /// Get all Part tokens from this serial
    /// Returns (index, values) pairs for each Part token
    pub fn parts(&self) -> Vec<(u64, Vec<u64>)> {
        self.tokens
            .iter()
            .filter_map(|t| {
                if let Token::Part { index, values } = t {
                    Some((*index, values.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Display detailed byte-by-byte breakdown
    pub fn detailed_dump(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("Serial: {}\n", self.original));
        output.push_str(&format!("Item type: {}\n", self.item_type));
        output.push_str(&format!("Bytes: {} total\n\n", self.raw_bytes.len()));

        // Show extracted fields
        output.push_str("Extracted fields:\n");
        if let Some(m) = self.manufacturer {
            output.push_str(&format!("  Manufacturer: {}\n", m));
        }
        if let Some(s) = self.seed {
            output.push_str(&format!("  Seed: {}\n", s));
        }
        if let Some(l) = self.level {
            output.push_str(&format!("  Level: {}\n", l));
        }
        output.push('\n');

        // Show parsed tokens
        output.push_str(&format!("Tokens: {} total\n", self.tokens.len()));
        for (i, token) in self.tokens.iter().enumerate() {
            output.push_str(&format!("  [{:2}] {:?}\n", i, token));
        }
        output.push('\n');

        // Show raw bytes
        output.push_str("Raw bytes:\n");
        for (i, byte) in self.raw_bytes.iter().enumerate() {
            output.push_str(&format!(
                "[{:3}] = {:3} (0x{:02x}) (0b{:08b})\n",
                i, byte, byte, byte
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_weapon_serial() {
        // Example weapon serial from memory dump
        let serial = "@Ugr$ZCm/&tH!t{KgK/Shxu>k";
        let item = ItemSerial::decode(serial).unwrap();

        assert_eq!(item.item_type, 'r');
        assert!(!item.raw_bytes.is_empty());
        assert!(!item.tokens.is_empty(), "Should parse at least one token");

        // First byte should be 0x21 (contains magic header 0010000)
        assert_eq!(item.raw_bytes[0], 0x21);
    }

    #[test]
    fn test_decode_equipment_serial() {
        // Equipment serial from save file
        let serial = "@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_";
        let item = ItemSerial::decode(serial).unwrap();

        assert_eq!(item.item_type, 'e');
        assert!(!item.raw_bytes.is_empty());
        assert_eq!(item.raw_bytes[0], 0x21); // Magic header
    }

    #[test]
    fn test_decode_utility_serial() {
        // Utility item - first VarInt is item subtype identifier
        let serial = "@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1";
        let item = ItemSerial::decode(serial).unwrap();

        assert_eq!(item.item_type, 'u');
        assert!(!item.tokens.is_empty());

        // First VarInt(128) is the item subtype identifier for utility items
        assert_eq!(item.manufacturer, Some(128));
    }

    #[test]
    fn test_invalid_serial_prefix() {
        let result = ItemSerial::decode("InvalidSerial");
        assert!(result.is_err());
    }

    #[test]
    fn test_base85_decode() {
        // Test basic Base85 decoding
        let result = decode_base85("g").unwrap();
        assert_eq!(result.len(), 0); // Single char decodes to 0 bytes with partial chunk
    }

    #[test]
    fn test_mirror_byte() {
        assert_eq!(mirror_byte(0b10000000), 0b00000001);
        assert_eq!(mirror_byte(0b11000000), 0b00000011);
        assert_eq!(mirror_byte(0b10101010), 0b01010101);
        assert_eq!(mirror_byte(0b00000000), 0b00000000);
        assert_eq!(mirror_byte(0b11111111), 0b11111111);
    }

    #[test]
    fn test_part_group_id_extraction() {
        // Weapon serial - Vladof SMG (group 22)
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        assert_eq!(item.part_group_id(), Some(22));

        // Equipment serial - Shield (group 279)
        let item = ItemSerial::decode("@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_").unwrap();
        assert_eq!(item.part_group_id(), Some(279));

        // Utility items don't use Part Group ID
        let item =
            ItemSerial::decode("@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1").unwrap();
        assert_eq!(item.part_group_id(), None);
    }

    #[test]
    fn test_parts_extraction() {
        // Weapon with one part
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let parts = item.parts();
        assert!(!parts.is_empty(), "Should have at least one part");

        // Check first part has index 0 and value
        let (index, values) = &parts[0];
        assert_eq!(*index, 0u64);
        assert!(!values.is_empty());
    }

    #[test]
    fn test_equipment_level_extraction() {
        // Shield type-e at level 49
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        assert_eq!(item.level, Some(49));

        // Grenade at level 49
        let item = ItemSerial::decode("@Uge8Xtm/)}}!elF;NmXinbwH6?9}OPi1ON").unwrap();
        assert_eq!(item.level, Some(49));

        // Class mod at level 50
        let item = ItemSerial::decode("@Uge8;)m/)@{!X>!SqTZJibf`hSk4B2r6#)").unwrap();
        assert_eq!(item.level, Some(50));

        // Shield type-r at level 49
        let item = ItemSerial::decode("@Ugr$)Nm/%P$!bIqxL{(~iG&p36L=sIx00").unwrap();
        assert_eq!(item.level, Some(49));

        // Weapon still works - level 30
        let item = ItemSerial::decode("@Ugb)KvFg_4rJ}%H-RG}IbsZG^E#X_Y-00").unwrap();
        assert_eq!(item.level, Some(30));
    }
}
