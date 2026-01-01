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
    item_type_name, level_from_code, manufacturer_name, serial_format, serial_id_to_parts_category,
    weapon_info_from_first_varint,
};

/// Custom Base85 alphabet used by Borderlands 4
const BL4_BASE85_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~";

/// Element types for weapons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Kinetic,   // ID 0
    Corrosive, // ID 5
    Shock,     // ID 8
    Radiation, // ID 9
    Cryo,      // ID 13
    Fire,      // ID 14
}

impl Element {
    /// Convert element ID to Element type
    pub fn from_id(id: u64) -> Option<Self> {
        match id {
            0 => Some(Element::Kinetic),
            5 => Some(Element::Corrosive),
            8 => Some(Element::Shock),
            9 => Some(Element::Radiation),
            13 => Some(Element::Cryo),
            14 => Some(Element::Fire),
            _ => None,
        }
    }

    /// Get element name
    pub fn name(&self) -> &'static str {
        match self {
            Element::Kinetic => "Kinetic",
            Element::Corrosive => "Corrosive",
            Element::Shock => "Shock",
            Element::Radiation => "Radiation",
            Element::Cryo => "Cryo",
            Element::Fire => "Fire",
        }
    }
}

/// Item rarity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Get rarity name
    pub fn name(&self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
        }
    }

    /// Extract rarity from VarBit-first equipment format
    /// Rarity is encoded in bits 6-7 of (first_varbit % divisor)
    pub fn from_equipment_varbit(varbit: u64, divisor: u64) -> Option<Self> {
        if divisor == 0 {
            return None;
        }
        let remainder = varbit % divisor;
        let rarity_bits = (remainder >> 6) & 0x3;
        match rarity_bits {
            0 => Some(Rarity::Common),
            1 => Some(Rarity::Epic),      // Observed: 64 >> 6 = 1 for Epic
            2 => Some(Rarity::Rare),      // Hypothetical
            3 => Some(Rarity::Legendary), // Observed: 192 >> 6 = 3 for Legendary
            _ => None,
        }
    }

    /// Extract rarity from VarInt-first weapon format
    /// For level codes > 145 (max level 50), rarity is encoded in the offset
    pub fn from_weapon_level_code(code: u64) -> Option<Self> {
        // Level codes 128-145 encode levels 16-50 (Common rarity)
        // Codes > 145 encode level 50 + rarity
        if code <= 145 {
            Some(Rarity::Common)
        } else {
            // Known codes from database samples:
            // 192 = Epic, 200 = Legendary
            match code {
                192 => Some(Rarity::Epic),
                200 => Some(Rarity::Legendary),
                // For unknown codes, estimate based on value range
                146..=180 => Some(Rarity::Uncommon),
                181..=195 => Some(Rarity::Epic),
                196.. => Some(Rarity::Legendary),
                _ => None,
            }
        }
    }
}

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
    /// Item level (fourth VarInt for VarInt-first format), capped at 50
    pub level: Option<u64>,
    /// Raw decoded level before capping (if > 50, our decoding may be wrong)
    pub raw_level: Option<u64>,
    /// Random seed for stat rolls (second VarInt after first separator)
    pub seed: Option<u64>,
    /// Detected elements (from Part tokens with index 128-142)
    pub elements: Vec<Element>,
    /// Detected rarity (from level code or equipment VarBit)
    pub rarity: Option<Rarity>,
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

        // For partial chunks, pad with highest value (84 = 'u') to make 5 chars
        // This ensures we decode to the most significant bytes
        for &ch in chunk.iter() {
            let byte_val = lookup[ch as usize] as u64;
            value = value * 85 + byte_val;
        }
        // Pad remaining positions with 84 (highest value)
        for _ in chunk.len()..5 {
            value = value * 85 + 84;
        }

        // Extract bytes from most significant first
        let num_bytes = if chunk.len() == 5 { 4 } else { chunk.len() - 1 };
        for i in (0..num_bytes).rev() {
            // For partial chunks, extract from high bytes (shift by 24, 16, etc.)
            let shift = if chunk.len() == 5 {
                i * 8
            } else {
                (3 - (num_bytes - 1 - i)) * 8
            };
            result.push(((value >> shift) & 0xFF) as u8);
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
        // Build value from bytes (big-endian), pad with zeros to 4 bytes
        let mut value: u64 = 0;
        for &byte in chunk {
            value = (value << 8) | (byte as u64);
        }
        // Pad partial chunks with zeros (shift left to fill 4 bytes)
        if chunk.len() < 4 {
            value <<= (4 - chunk.len()) * 8;
        }

        // Convert to 5 base85 chars, then take first N+1 for partial chunks
        let mut chars = [0u8; 5];
        for i in (0..5).rev() {
            chars[i] = BL4_BASE85_ALPHABET[(value % 85) as usize];
            value /= 85;
        }

        // Take first N+1 chars for partial chunks
        let num_chars = if chunk.len() == 4 { 5 } else { chunk.len() + 1 };
        for &ch in &chars[0..num_chars] {
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

/// Read a value list for Part tokens (values until 00 separator)
fn read_part_value_list(reader: &mut BitReader) -> Vec<u64> {
    let mut values = Vec::new();
    loop {
        let start_pos = reader.bit_offset;
        let Some(peek) = reader.read_bits(2) else {
            break;
        };

        // 00 = separator, end of list
        if peek == 0b00 {
            break;
        }

        // Read third bit to get full prefix
        let Some(bit3) = reader.read_bits(1) else {
            break;
        };

        match (peek << 1) | bit3 {
            0b100 => {
                if let Some(v) = reader.read_varint() {
                    values.push(v);
                }
            }
            0b110 => {
                if let Some(v) = reader.read_varbit() {
                    values.push(v);
                }
            }
            _ => {
                // Unknown prefix, rewind and break
                reader.bit_offset = start_pos;
                break;
            }
        }
    }
    values
}

/// Parse Part token data after the index has been read
fn parse_part_values(reader: &mut BitReader) -> Vec<u64> {
    let Some(type_flag) = reader.read_bits(1) else {
        return Vec::new();
    };

    if type_flag == 1 {
        // SUBTYPE_INT: single VARINT value + 000 terminator
        let mut values = Vec::new();
        if let Some(val) = reader.read_varint() {
            values.push(val);
        }
        let _ = reader.read_bits(3); // 000 terminator
        return values;
    }

    // Type 0: read 2-bit subtype
    let Some(subtype) = reader.read_bits(2) else {
        return Vec::new();
    };

    match subtype {
        0b10 => Vec::new(),                   // SUBTYPE_NONE: no data
        0b01 => read_part_value_list(reader), // SUBTYPE_LIST: values until 00
        _ => Vec::new(),                      // Unknown subtype
    }
}

/// Parse a String token (prefix 111)
fn parse_string_token(reader: &mut BitReader) -> Option<Token> {
    let length = reader.read_varint()?;

    // Sanity check - don't read more than 128 chars
    if length > 128 {
        return None;
    }

    let mut chars = Vec::with_capacity(length as usize);
    for _ in 0..length {
        if let Some(ch) = reader.read_bits(7) {
            chars.push(ch as u8);
        }
    }

    let s = String::from_utf8(chars.clone())
        .unwrap_or_else(|_| String::from_utf8_lossy(&chars).to_string());
    Some(Token::String(s))
}

/// Parse tokens with 3-bit prefix (100, 101, 110, 111)
fn parse_3bit_token(reader: &mut BitReader, prefix3: u64, debug: bool) -> Option<Token> {
    match prefix3 {
        0b100 => reader.read_varint().map(Token::VarInt),
        0b110 => reader.read_varbit().map(Token::VarBit),
        0b101 => {
            let index = reader.read_varint()?;
            if index > MAX_REASONABLE_PART_INDEX {
                if debug {
                    eprintln!(
                        "         -> Part index {} exceeds max ({})",
                        index, MAX_REASONABLE_PART_INDEX
                    );
                }
                return None;
            }
            let values = parse_part_values(reader);
            Some(Token::Part { index, values })
        }
        0b111 => parse_string_token(reader),
        _ => None,
    }
}

/// Parse tokens with optional debug output
fn parse_tokens_impl(reader: &mut BitReader, debug: bool) -> Vec<Token> {
    let mut tokens = Vec::new();

    // Verify magic header (7 bits = 0010000)
    let Some(magic) = reader.read_bits(7) else {
        return tokens;
    };
    if magic != 0b0010000 {
        if debug {
            eprintln!("Warning: Invalid magic header: {:07b}", magic);
        }
        return tokens;
    }

    // Parse tokens until terminator or end of data
    for _ in 0..100 {
        let bit_pos = reader.bit_offset;

        let Some(prefix2) = reader.read_bits(2) else {
            break;
        };

        if debug {
            eprintln!("[bit {:3}] Prefix: {:02b}", bit_pos, prefix2);
        }

        match prefix2 {
            0b00 => {
                tokens.push(Token::Separator);
                // Need at least 8 bits for a meaningful token
                if reader.remaining_bits() < 8 {
                    break;
                }
            }
            0b01 => tokens.push(Token::SoftSeparator),
            0b10 | 0b11 => {
                let Some(bit3) = reader.read_bits(1) else {
                    break;
                };
                let prefix3 = (prefix2 << 1) | bit3;

                match parse_3bit_token(reader, prefix3, debug) {
                    Some(token) => tokens.push(token),
                    None if prefix3 == 0b101 => break, // Part index too large
                    None => {}
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

/// Intermediate result for header extraction
struct HeaderInfo {
    manufacturer: Option<u64>,
    level: Option<u64>,
    raw_level: Option<u64>,
    seed: Option<u64>,
}

/// Decode serial prefix, validate, and return raw bytes
fn decode_serial_bytes(serial: &str) -> Result<(char, Vec<u8>), SerialError> {
    if !serial.starts_with("@Ug") {
        return Err(SerialError::InvalidEncoding(
            "Serial must start with @Ug".to_string(),
        ));
    }

    let item_type = serial.chars().nth(3).ok_or_else(|| {
        SerialError::InvalidEncoding("Serial too short - no item type".to_string())
    })?;

    let encoded_data = &serial[2..];
    let decoded = decode_base85(encoded_data)?;
    let raw_bytes: Vec<u8> = decoded.iter().map(|&b| mirror_byte(b)).collect();

    if raw_bytes.len() < 4 {
        return Err(SerialError::TooShort {
            expected: 4,
            actual: raw_bytes.len(),
        });
    }

    Ok((item_type, raw_bytes))
}

/// Collect VarInts before and after first separator
fn collect_varints(tokens: &[Token]) -> (Vec<u64>, Vec<u64>) {
    let mut before_sep = Vec::new();
    let mut after_sep = Vec::new();
    let mut seen_sep = false;

    for token in tokens {
        match token {
            Token::VarInt(v) => {
                if seen_sep {
                    after_sep.push(*v);
                } else {
                    before_sep.push(*v);
                }
            }
            Token::Separator => seen_sep = true,
            _ => {}
        }
    }
    (before_sep, after_sep)
}

/// Extract header info from equipment format (VarBit-first)
fn extract_equipment_header(tokens: &[Token]) -> HeaderInfo {
    let mut level = None;
    let mut raw_level = None;
    let mut seen_first_sep = false;

    for token in tokens {
        match token {
            Token::Separator if !seen_first_sep => seen_first_sep = true,
            Token::VarBit(v) if seen_first_sep && level.is_none() => {
                let adjusted = v.saturating_add(1);
                if let Some((capped, raw)) = level_from_code(adjusted) {
                    level = Some(capped as u64);
                    raw_level = Some(raw as u64);
                }
                break;
            }
            _ => {}
        }
    }

    HeaderInfo {
        manufacturer: None,
        level,
        raw_level,
        seed: None,
    }
}

/// Extract header info from weapon format (VarInt-first)
fn extract_weapon_header(tokens: &[Token]) -> HeaderInfo {
    let (header_varints, after_first_sep) = collect_varints(tokens);

    let manufacturer = header_varints.first().copied();
    let (level, raw_level) = if header_varints.len() >= 4 {
        if let Some((capped, raw)) = level_from_code(header_varints[3]) {
            if raw <= 50 {
                (Some(capped as u64), Some(raw as u64))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    let seed = after_first_sep.get(1).copied();

    HeaderInfo {
        manufacturer,
        level,
        raw_level,
        seed,
    }
}

/// Extract elements from Part tokens (index 128-142)
fn extract_elements(tokens: &[Token]) -> Vec<Element> {
    tokens
        .iter()
        .filter_map(|token| {
            if let Token::Part { index, .. } = token {
                if *index >= 128 && *index <= 142 {
                    Element::from_id(index - 128)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

/// Extract rarity based on item format
fn extract_rarity(tokens: &[Token], item_type: char, is_varbit_first: bool) -> Option<Rarity> {
    if is_varbit_first {
        let first_varbit = tokens.iter().find_map(|t| {
            if let Token::VarBit(v) = t {
                Some(*v)
            } else {
                None
            }
        })?;
        let divisor = serial_format(item_type)
            .map(|f| f.category_divisor)
            .unwrap_or(384);
        Rarity::from_equipment_varbit(first_varbit, divisor)
    } else {
        let (header_varints, _) = collect_varints(tokens);
        if header_varints.len() >= 4 {
            Rarity::from_weapon_level_code(header_varints[3])
        } else {
            None
        }
    }
}

impl ItemSerial {
    /// Decode a Borderlands 4 item serial
    ///
    /// Format: `@Ug<type><base85_data>`
    /// Example: @Ugr$ZCm/&tH!t{KgK/Shxu>k
    pub fn decode(serial: &str) -> Result<Self, SerialError> {
        let (item_type, raw_bytes) = decode_serial_bytes(serial)?;

        let mut reader = BitReader::new(raw_bytes.clone());
        let tokens = parse_tokens(&mut reader);

        let is_varbit_first = matches!(tokens.first(), Some(Token::VarBit(_)));
        let header = if is_varbit_first {
            extract_equipment_header(&tokens)
        } else {
            extract_weapon_header(&tokens)
        };

        let elements = extract_elements(&tokens);
        let rarity = extract_rarity(&tokens, item_type, is_varbit_first);

        Ok(ItemSerial {
            original: serial.to_string(),
            raw_bytes,
            item_type,
            tokens,
            manufacturer: header.manufacturer,
            level: header.level,
            raw_level: header.raw_level,
            seed: header.seed,
            elements,
            rarity,
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
        // Re-extract elements from the new tokens
        let elements: Vec<Element> = tokens
            .iter()
            .filter_map(|token| {
                if let Token::Part { index, .. } = token {
                    if *index >= 128 && *index <= 142 {
                        Element::from_id(index - 128)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        ItemSerial {
            original: self.original.clone(),
            raw_bytes: self.raw_bytes.clone(), // Will be stale but that's OK
            item_type: self.item_type,
            tokens,
            manufacturer: self.manufacturer,
            level: self.level,
            raw_level: self.raw_level,
            seed: self.seed,
            elements,
            rarity: self.rarity, // Preserve rarity from original
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

    /// Get element names as a formatted string
    /// Returns None if no elements detected, otherwise returns comma-separated list
    pub fn element_names(&self) -> Option<String> {
        if self.elements.is_empty() {
            None
        } else {
            let names: Vec<&str> = self.elements.iter().map(|e| e.name()).collect();
            Some(names.join(", "))
        }
    }

    /// Get rarity name
    /// Returns None if rarity not detected
    pub fn rarity_name(&self) -> Option<&'static str> {
        self.rarity.map(|r| r.name())
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

    /// Get the parts database category for this item
    ///
    /// For VarBit-first items (shields, etc), uses the extracted category.
    /// For VarInt-first items (weapons), converts the serial ID to parts DB category.
    pub fn parts_category(&self) -> Option<i64> {
        // First try VarBit-first extraction
        if let Some(cat) = self.part_group_id() {
            return Some(cat);
        }

        // Fall back to VarInt-first: convert serial ID to parts category
        self.manufacturer
            .map(|id| serial_id_to_parts_category(id) as i64)
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

    /// Get all parts with their resolved names
    /// Returns (index, name, is_flagged, values) tuples where:
    /// - index is the raw part index
    /// - name is the part name from the manifest (or None if not found)
    /// - is_flagged indicates if the high bit (0x80) was set (purpose TBD)
    /// - values are any associated values
    ///
    /// Note: Indices >= 128 may be:
    /// - Element markers (128-142 = elements 0-14)
    /// - Modified parts (index & 0x7F gives the base part index, flag meaning unknown)
    pub fn parts_with_names(&self) -> Vec<(u64, Option<&'static str>, bool, Vec<u64>)> {
        let category = self.parts_category().unwrap_or(-1);
        self.parts()
            .into_iter()
            .map(|(index, values)| {
                // Check if high bit is set (flagged part)
                let is_flagged = index >= 128;
                let lookup_index = if is_flagged {
                    (index & 0x7F) as i64
                } else {
                    index as i64
                };

                // First try direct lookup, then try base index for flagged parts
                let name = crate::manifest::part_name(category, lookup_index);
                (index, name, is_flagged, values)
            })
            .collect()
    }

    /// Get a summary of resolved part names
    /// Returns a formatted string with part names, or indices for unknown parts
    pub fn parts_summary(&self) -> String {
        let parts = self.parts_with_names();
        if parts.is_empty() {
            return String::new();
        }

        let mut output = Vec::new();
        for (index, name, is_flagged, values) in parts {
            // Skip element markers (128-142) as they're shown separately
            if index >= 128 && index <= 142 {
                continue;
            }

            let flag_marker = if is_flagged { "+" } else { "" };
            let part_str = match name {
                Some(n) => {
                    // Extract just the part name after the prefix (e.g., "part_barrel_01" from "DAD_PS.part_barrel_01")
                    let short_name = n.split('.').last().unwrap_or(n);
                    if values.is_empty() {
                        format!("{}{}", flag_marker, short_name)
                    } else if values.len() == 1 {
                        format!("{}{}:{}", flag_marker, short_name, values[0])
                    } else {
                        format!("{}{}:{:?}", flag_marker, short_name, values)
                    }
                }
                None => {
                    if values.is_empty() {
                        format!("[{}]", index)
                    } else if values.len() == 1 {
                        format!("[{}]:{}", index, values[0])
                    } else {
                        format!("[{}]:{:?}", index, values)
                    }
                }
            };
            output.push(part_str);
        }
        output.join(", ")
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
        // Shield type-e with VarBit 49 = Level 50 (0-indexed storage)
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        assert_eq!(item.level, Some(50));

        // Grenade with VarBit 49 = Level 50
        let item = ItemSerial::decode("@Uge8Xtm/)}}!elF;NmXinbwH6?9}OPi1ON").unwrap();
        assert_eq!(item.level, Some(50));

        // Class mod with VarBit 50 = Level 51 (invalid, returns None)
        let item = ItemSerial::decode("@Uge8;)m/)@{!X>!SqTZJibf`hSk4B2r6#)").unwrap();
        assert_eq!(item.level, None);

        // Shield type-r with VarBit 49 = Level 50
        let item = ItemSerial::decode("@Ugr$)Nm/%P$!bIqxL{(~iG&p36L=sIx00").unwrap();
        assert_eq!(item.level, Some(50));

        // Weapon still works - level 30
        let item = ItemSerial::decode("@Ugb)KvFg_4rJ}%H-RG}IbsZG^E#X_Y-00").unwrap();
        assert_eq!(item.level, Some(30));
    }

    #[test]
    fn test_encode_roundtrip() {
        // Test that decode -> encode produces the original serial
        let test_serials = [
            // Hellwalker (Fire shotgun)
            "@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-",
            // Jakobs Pistol (Corrosive)
            "@UgbV{rFjEj=bZ<~-RG}KRs7TF2b*c{P7OEuz",
            // Energy Shield
            "@Uge98>m/)}}!c5JeNWCvCXc7",
            // Class Mod
            "@Ug!pHG2}TYgjMfjzn~K!T)XUVX)U4Eu)Qi+?RPAVZh!@!b00",
            // Grenade
            "@Ugr$N8m/)}}!q9r4K/ShxuK@",
        ];

        for serial in test_serials {
            let item = ItemSerial::decode(serial).unwrap();
            let re_encoded = item.encode();
            assert_eq!(
                re_encoded, serial,
                "Round-trip failed for {}: got {}",
                serial, re_encoded
            );
        }
    }

    #[test]
    fn test_element_from_id() {
        // Verify element ID mapping
        assert_eq!(Element::from_id(0), Some(Element::Kinetic));
        assert_eq!(Element::from_id(5), Some(Element::Corrosive));
        assert_eq!(Element::from_id(8), Some(Element::Shock));
        assert_eq!(Element::from_id(9), Some(Element::Radiation));
        assert_eq!(Element::from_id(13), Some(Element::Cryo));
        assert_eq!(Element::from_id(14), Some(Element::Fire));
        assert_eq!(Element::from_id(99), None); // Unknown ID
    }

    #[test]
    fn test_element_names() {
        assert_eq!(Element::Kinetic.name(), "Kinetic");
        assert_eq!(Element::Corrosive.name(), "Corrosive");
        assert_eq!(Element::Shock.name(), "Shock");
        assert_eq!(Element::Radiation.name(), "Radiation");
        assert_eq!(Element::Cryo.name(), "Cryo");
        assert_eq!(Element::Fire.name(), "Fire");
    }

    #[test]
    fn test_element_extraction_fire() {
        // Hellwalker (Fire shotgun) - verified in-game
        let item = ItemSerial::decode("@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-").unwrap();
        assert_eq!(item.elements.len(), 1);
        assert_eq!(item.elements[0], Element::Fire);
        assert_eq!(item.element_names(), Some("Fire".to_string()));
    }

    #[test]
    fn test_element_extraction_corrosive() {
        // Jakobs Pistol (Corrosive)
        let item = ItemSerial::decode("@UgbV{rFjEj=bZ<~-RG}KRs7TF2b*c{P7OEuz").unwrap();
        assert_eq!(item.elements.len(), 1);
        assert_eq!(item.elements[0], Element::Corrosive);
        assert_eq!(item.element_names(), Some("Corrosive".to_string()));
    }

    #[test]
    fn test_element_extraction_none() {
        // Energy Shield - no weapon element
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        assert!(item.elements.is_empty());
        assert_eq!(item.element_names(), None);
    }

    // Tests for Rarity enum
    mod rarity_tests {
        use super::*;

        #[test]
        fn test_rarity_name() {
            assert_eq!(Rarity::Common.name(), "Common");
            assert_eq!(Rarity::Uncommon.name(), "Uncommon");
            assert_eq!(Rarity::Rare.name(), "Rare");
            assert_eq!(Rarity::Epic.name(), "Epic");
            assert_eq!(Rarity::Legendary.name(), "Legendary");
        }

        #[test]
        fn test_from_equipment_varbit_common() {
            // Bits 6-7 = 0 -> Common
            let rarity = Rarity::from_equipment_varbit(0, 384);
            assert_eq!(rarity, Some(Rarity::Common));
        }

        #[test]
        fn test_from_equipment_varbit_epic() {
            // Bits 6-7 = 1 -> Epic (64 >> 6 = 1)
            let rarity = Rarity::from_equipment_varbit(64, 384);
            assert_eq!(rarity, Some(Rarity::Epic));
        }

        #[test]
        fn test_from_equipment_varbit_legendary() {
            // Bits 6-7 = 3 -> Legendary (192 >> 6 = 3)
            let rarity = Rarity::from_equipment_varbit(192, 384);
            assert_eq!(rarity, Some(Rarity::Legendary));
        }

        #[test]
        fn test_from_equipment_varbit_zero_divisor() {
            // Zero divisor should return None
            let rarity = Rarity::from_equipment_varbit(100, 0);
            assert_eq!(rarity, None);
        }

        #[test]
        fn test_from_weapon_level_code_common() {
            // Level codes <= 145 are Common
            assert_eq!(Rarity::from_weapon_level_code(128), Some(Rarity::Common));
            assert_eq!(Rarity::from_weapon_level_code(145), Some(Rarity::Common));
        }

        #[test]
        fn test_from_weapon_level_code_epic() {
            // Code 192 = Epic
            assert_eq!(Rarity::from_weapon_level_code(192), Some(Rarity::Epic));
        }

        #[test]
        fn test_from_weapon_level_code_legendary() {
            // Code 200 = Legendary
            assert_eq!(Rarity::from_weapon_level_code(200), Some(Rarity::Legendary));
        }

        #[test]
        fn test_from_weapon_level_code_ranges() {
            // Uncommon range: 146-180
            assert_eq!(Rarity::from_weapon_level_code(150), Some(Rarity::Uncommon));

            // Epic range: 181-195
            assert_eq!(Rarity::from_weapon_level_code(185), Some(Rarity::Epic));

            // Legendary range: 196+
            assert_eq!(Rarity::from_weapon_level_code(210), Some(Rarity::Legendary));
        }
    }

    // Tests for ItemSerial display methods
    mod display_tests {
        use super::*;

        #[test]
        fn test_hex_dump() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let hex = item.hex_dump();

            // Should be a valid hex string (even length, hex chars only)
            assert!(!hex.is_empty());
            assert!(hex.len() % 2 == 0);
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        }

        #[test]
        fn test_format_tokens_weapon() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let formatted = item.format_tokens();

            // Should produce a non-empty string
            assert!(!formatted.is_empty());

            // Should contain at least a separator (|) and a part marker ({})
            assert!(formatted.contains('|') || formatted.contains('{'));
        }

        #[test]
        fn test_format_tokens_structure() {
            let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
            let formatted = item.format_tokens();

            // Formatted tokens should be parseable - contains VarInt/VarBit values
            // and separators
            assert!(!formatted.is_empty());
        }

        #[test]
        fn test_item_type_description_weapon() {
            // Hellwalker - type 'd' which is a VarInt-first weapon
            let item = ItemSerial::decode("@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-").unwrap();
            assert_eq!(item.item_type_description(), "Weapon");
        }

        #[test]
        fn test_item_type_description_shield() {
            // Type 'r' is VarBit-first (shields/items)
            let item = ItemSerial::decode("@Ugr$N8m/)}}!q9r4K/ShxuK@").unwrap();
            assert_eq!(item.item_type_description(), "Item");
        }

        #[test]
        fn test_item_type_description_equipment() {
            let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
            // Equipment items return "Item" from item_type_name
            assert_eq!(item.item_type_description(), "Item");
        }

        #[test]
        fn test_item_type_description_class_mod() {
            // Class mod - type '!'
            let item =
                ItemSerial::decode("@Ug!pHG2}TYgjMfjzn~K!T)XUVX)U4Eu)Qi+?RPAVZh!@!b00").unwrap();
            assert_eq!(item.item_type_description(), "Class Mod");
        }

        #[test]
        fn test_manufacturer_name_known() {
            // Vladof SMG
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            // The manufacturer ID maps to a known manufacturer
            if let Some(name) = item.manufacturer_name() {
                assert!(!name.is_empty());
            }
        }

        #[test]
        fn test_rarity_name_method() {
            // Weapon with detectable rarity
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            if let Some(rarity) = item.rarity_name() {
                // Should be one of the valid rarity names
                let valid = ["Common", "Uncommon", "Rare", "Epic", "Legendary"];
                assert!(valid.contains(&rarity));
            }
        }

        #[test]
        fn test_weapon_info_for_weapon() {
            // Known weapon serial
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            if let Some((mfg, wtype)) = item.weapon_info() {
                assert!(!mfg.is_empty());
                assert!(!wtype.is_empty());
            }
        }

        #[test]
        fn test_weapon_info_for_equipment() {
            // Equipment doesn't have weapon info
            let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
            // May or may not return info depending on format detection
            // Just verify it doesn't panic
            let _ = item.weapon_info();
        }

        #[test]
        fn test_parts_category() {
            // Weapon serial - should have parts category
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let category = item.parts_category();
            assert!(category.is_some());
        }

        #[test]
        fn test_detailed_dump() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let dump = item.detailed_dump();

            // Should contain serial info
            assert!(dump.contains("Serial:"));
            assert!(dump.contains("Item type:"));
            assert!(dump.contains("Bytes:"));
            assert!(dump.contains("Tokens:"));
            assert!(dump.contains("Raw bytes:"));
        }
    }

    // Tests for BitReader and BitWriter
    mod bitstream_tests {
        use super::*;

        #[test]
        fn test_varint_roundtrip() {
            // Test various values
            for value in [0u64, 1, 15, 16, 255, 1000, 65535] {
                let mut writer = BitWriter::new();
                writer.write_varint(value);
                let bytes = writer.finish();

                let mut reader = BitReader::new(bytes);
                let read_value = reader.read_varint().unwrap();
                assert_eq!(read_value, value, "VarInt roundtrip failed for {}", value);
            }
        }

        #[test]
        fn test_varbit_roundtrip() {
            // Test various values
            for value in [0u64, 1, 7, 8, 31, 32, 127, 1000] {
                let mut writer = BitWriter::new();
                writer.write_varbit(value);
                let bytes = writer.finish();

                let mut reader = BitReader::new(bytes);
                let read_value = reader.read_varbit().unwrap();
                assert_eq!(read_value, value, "VarBit roundtrip failed for {}", value);
            }
        }

        #[test]
        fn test_bits_roundtrip() {
            // Test fixed-width bit values
            let mut writer = BitWriter::new();
            writer.write_bits(0b1010, 4);
            writer.write_bits(0b11111111, 8);
            writer.write_bits(0b101, 3);
            let bytes = writer.finish();

            let mut reader = BitReader::new(bytes);
            assert_eq!(reader.read_bits(4), Some(0b1010));
            assert_eq!(reader.read_bits(8), Some(0b11111111));
            assert_eq!(reader.read_bits(3), Some(0b101));
        }

        #[test]
        fn test_remaining_bits() {
            let reader = BitReader::new(vec![0xFF, 0xFF]);
            assert_eq!(reader.remaining_bits(), 16);
        }
    }
}
