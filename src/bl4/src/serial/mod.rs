//! Item serial number decoding for Borderlands 4
//!
//! Item serials use a custom Base85 encoding with bit-packed data.
//!
//! Format:
//! 1. Serials start with `@U` prefix
//! 2. Encoded with custom Base85 alphabet
//! 3. Decoded bytes have mirrored bits
//! 4. Data is a variable-length bitstream with tokens

mod base85;
mod bitstream;
mod rarity;
pub mod resolve;
mod validate;

use base85::{decode_base85, encode_base85, mirror_byte};
use bitstream::{BitReader, BitWriter};

pub use rarity::RarityEstimate;
pub use validate::{Legality, ValidationCheck, ValidationResult};

use crate::manifest::SHARED_VERTICAL_CATEGORIES;
use crate::parts::{
    category_from_varbit, level_from_code, manufacturer_name, serial_id_to_parts_category,
    varbit_divisor, weapon_info_from_first_varint,
};

/// Element types for weapons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Kinetic,   // ID 0
    Corrosive, // ID 5
    Shock,     // ID 8
    Radiation, // ID 9
    Sonic,     // ID unknown — confirmed in balance schemas, no serial data yet
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

    /// Return the numeric ID for this element
    pub fn to_id(&self) -> u64 {
        match self {
            Element::Kinetic => 0,
            Element::Corrosive => 5,
            Element::Shock => 8,
            Element::Radiation => 9,
            Element::Cryo => 13,
            Element::Fire => 14,
            Element::Sonic => 15, // Provisional — no serial data yet
        }
    }

    /// Return the Part token index for this element.
    ///
    /// Element markers in the Part token stream use indices in the 16-27 range.
    pub fn to_index(&self) -> u64 {
        match self {
            Element::Kinetic => 16,   // was 128
            Element::Corrosive => 26, // was 133
            Element::Shock => 17,     // was 136
            Element::Radiation => 25, // was 137
            Element::Cryo => 27,      // was 141
            Element::Fire => 23,      // was 142
            Element::Sonic => 31,     // was 143 (provisional)
        }
    }

    /// Convert a Part token index to an Element, if it maps to a known element.
    /// Detect element from a resolved part name.
    ///
    /// Element parts are named `part_<element>`, `part_body_ele_*`, or `part_kinetic`.
    pub fn from_part_name(name: &str) -> Option<Self> {
        let base = name.split('.').next_back().unwrap_or(name);
        match base {
            "part_kinetic" => Some(Element::Kinetic),
            "part_corrosive" => Some(Element::Corrosive),
            "part_shock" => Some(Element::Shock),
            "part_radiation" => Some(Element::Radiation),
            "part_cryo" => Some(Element::Cryo),
            "part_fire" => Some(Element::Fire),
            _ if base.starts_with("part_body_ele_") => {
                // Multi-element parts (rainbowvomit) — report first element found
                if base.contains("cor") {
                    Some(Element::Corrosive)
                } else if base.contains("cryo") {
                    Some(Element::Cryo)
                } else if base.contains("fire") {
                    Some(Element::Fire)
                } else if base.contains("rad") {
                    Some(Element::Radiation)
                } else if base.contains("shock") {
                    Some(Element::Shock)
                } else {
                    Some(Element::Kinetic)
                }
            }
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
            Element::Sonic => "Sonic",
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

    pub fn from_bits(bits: u64) -> Option<Self> {
        match bits {
            0 => Some(Rarity::Common),
            1 => Some(Rarity::Epic),      // Observed: 64 >> 6 = 1 for Epic
            2 => Some(Rarity::Rare),      // Hypothetical
            3 => Some(Rarity::Legendary), // Observed: 192 >> 6 = 3 for Legendary
            _ => None,
        }
    }

    /// Extract rarity from VarBit-first equipment format.
    ///
    /// With correct bit ordering, the VarBit value is the category ID directly.
    /// Rarity for equipment items needs re-derivation from other token fields.
    pub fn from_equipment_varbit(_varbit: u64, _divisor: u64) -> Option<Self> {
        None
    }

    /// Extract rarity from VarInt-first weapon format.
    ///
    /// With correct bit ordering, level codes are just levels (1-50).
    /// Rarity is not encoded in the level code.
    /// Rarity extraction for weapons needs re-derivation.
    pub fn from_weapon_level_code(_code: u64) -> Option<Self> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenPrefix {
    VarInt = 0b100,
    Part = 0b101,
    VarBit = 0b110,
    String = 0b111,
}

impl TokenPrefix {
    fn from_bits(bits: u64) -> Option<Self> {
        match bits {
            0b100 => Some(TokenPrefix::VarInt),
            0b101 => Some(TokenPrefix::Part),
            0b110 => Some(TokenPrefix::VarBit),
            0b111 => Some(TokenPrefix::String),
            _ => None,
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
}

/// Serial encoding format, determined from the binary token stream.
///
/// The first token after decoding determines the format:
/// - `VarBitFirst`: first token is a VarBit (equipment, shields, some weapons)
/// - `VarIntFirst`: first token is a VarInt (weapons, class mods)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialFormat {
    VarBitFirst,
    VarIntFirst,
}

impl std::fmt::Display for SerialFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VarBitFirst => write!(f, "varbit"),
            Self::VarIntFirst => write!(f, "varint"),
        }
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

/// A fully-resolved part from a decoded serial.
#[derive(Debug, Clone)]
pub struct ResolvedPart {
    /// Raw part index from the serial bitstream
    pub index: u64,
    /// Full manifest name (e.g., "DAD_PS.part_barrel_02"), or None if unresolved
    pub name: Option<&'static str>,
    /// Display-ready short name (e.g., "part_barrel_02", element name, or "[index]")
    pub short_name: String,
    /// Part slot (barrel, grip, scope, element, unknown, etc.)
    pub slot: &'static str,
    /// True if this part index is an element marker (indices 128-142)
    pub is_element: bool,
}

/// A decoded string token containing a UE asset path.
#[derive(Debug, Clone)]
pub struct ResolvedString {
    /// Full UE asset path (e.g., "MAL_SM.comp_05_legendary_firework")
    pub asset_path: String,
    /// Last segment of the path for display
    pub short_name: String,
}

/// Decoded item serial information
#[derive(Debug, Clone)]
pub struct ItemSerial {
    /// Original ASCII85-encoded serial
    pub original: String,

    /// Raw decoded bytes
    pub raw_bytes: Vec<u8>,

    /// Encoding format (VarBit-first or VarInt-first), derived from the binary token stream
    pub format: SerialFormat,

    /// Parsed tokens from bitstream
    pub tokens: Vec<Token>,

    /// Bit offset where each token starts (parallel to tokens)
    pub token_bit_offsets: Vec<usize>,

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
                writer.write_bits(TokenPrefix::VarInt as u64, 3);
                writer.write_varint(*v);
            }
            Token::VarBit(v) => {
                writer.write_bits(TokenPrefix::VarBit as u64, 3);
                writer.write_varbit(*v);
            }
            Token::Part { index, values } => {
                writer.write_bits(TokenPrefix::Part as u64, 3);
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
                        writer.write_bits(TokenPrefix::VarInt as u64, 3);
                        writer.write_varint(*v);
                    }
                    writer.write_bits(0b00, 2); // Separator to terminate
                }
            }
            Token::String(s) => {
                writer.write_bits(TokenPrefix::String as u64, 3);
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
fn parse_tokens(reader: &mut BitReader) -> (Vec<Token>, Vec<usize>) {
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

const PART_TYPE_SINGLE: u64 = 1;
const PART_SUBTYPE_NONE: u64 = 0b10;
const PART_SUBTYPE_LIST: u64 = 0b01;

/// Parse Part token data after the index has been read
fn parse_part_values(reader: &mut BitReader) -> Vec<u64> {
    let Some(type_flag) = reader.read_bits(1) else {
        return Vec::new();
    };

    if type_flag == PART_TYPE_SINGLE {
        let mut values = Vec::new();
        if let Some(val) = reader.read_varint() {
            values.push(val);
        }
        let _ = reader.read_bits(3); // 000 terminator
        return values;
    }

    let Some(subtype) = reader.read_bits(2) else {
        return Vec::new();
    };

    match subtype {
        PART_SUBTYPE_NONE => Vec::new(),
        PART_SUBTYPE_LIST => read_part_value_list(reader),
        _ => Vec::new(),
    }
}

/// Reverse the bit order within an N-bit value (up to 8 bits).
#[inline]
fn mirror_bits(val: u8, width: u8) -> u8 {
    let mut result = 0u8;
    for i in 0..width {
        if val & (1 << i) != 0 {
            result |= 1 << (width - 1 - i);
        }
    }
    result
}

/// Read a VarInt with each 4-bit nibble bit-mirrored.
/// Used by String tokens where all sub-fields have reversed bit order.
fn read_mirrored_varint(reader: &mut BitReader) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0;

    for _ in 0..4 {
        let raw_nibble = reader.read_bits(4)? as u8;
        let nibble = mirror_bits(raw_nibble, 4) as u64;
        result |= nibble << shift;
        shift += 4;

        let cont = reader.read_bits(1)?;
        if cont == 0 {
            return Some(result);
        }
    }

    Some(result)
}

/// Parse a String token (prefix 111).
/// All data after the prefix is bit-mirrored: the VarInt length has each
/// 4-bit nibble reversed, and each 7-bit character has its bits reversed.
fn parse_string_token(reader: &mut BitReader) -> Option<Token> {
    let length = read_mirrored_varint(reader)?;

    if length > 128 {
        return None;
    }

    let mut chars = Vec::with_capacity(length as usize);
    for _ in 0..length {
        let raw = reader.read_bits(7)? as u8;
        chars.push(mirror_bits(raw, 7));
    }

    let s = String::from_utf8(chars.clone())
        .unwrap_or_else(|_| String::from_utf8_lossy(&chars).to_string());
    Some(Token::String(s))
}

/// Parse tokens with 3-bit prefix (100, 101, 110, 111)
fn parse_3bit_token(reader: &mut BitReader, prefix: TokenPrefix, debug: bool) -> Option<Token> {
    match prefix {
        TokenPrefix::VarInt => reader.read_varint().map(Token::VarInt),
        TokenPrefix::VarBit => reader.read_varbit().map(Token::VarBit),
        TokenPrefix::Part => {
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
        TokenPrefix::String => parse_string_token(reader),
    }
}

/// Parse tokens with optional debug output, returning tokens and their bit offsets.
fn verify_magic(reader: &mut BitReader, debug: bool) -> bool {
    let Some(magic) = reader.read_bits(7) else {
        return false;
    };
    if magic != 0b0010000 {
        if debug {
            eprintln!("Warning: Invalid magic header: {:07b}", magic);
        }
        return false;
    }
    true
}

fn parse_tokens_impl(reader: &mut BitReader, debug: bool) -> (Vec<Token>, Vec<usize>) {
    let mut tokens = Vec::new();
    let mut offsets = Vec::new();

    if !verify_magic(reader, debug) {
        return (tokens, offsets);
    }

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
                offsets.push(bit_pos);
                tokens.push(Token::Separator);
                // Need at least 8 bits for a meaningful token
                if reader.remaining_bits() < 8 {
                    break;
                }
            }
            0b01 => {
                offsets.push(bit_pos);
                tokens.push(Token::SoftSeparator);
            }
            0b10 | 0b11 => {
                let Some(bit3) = reader.read_bits(1) else {
                    break;
                };
                let prefix3 = (prefix2 << 1) | bit3;
                let Some(prefix) = TokenPrefix::from_bits(prefix3) else {
                    break;
                };

                let token = parse_3bit_token(reader, prefix, debug);

                match token {
                    Some(t) => {
                        offsets.push(bit_pos);
                        tokens.push(t);
                    }
                    None if prefix == TokenPrefix::Part => break,
                    None => {}
                }
            }
            _ => break,
        }
    }

    (tokens, offsets)
}

/// Debug version of parse_tokens that prints what it sees
/// Note: expects already-mirrored bytes (as stored in ItemSerial.raw_bytes)
pub fn parse_tokens_debug(bytes: &[u8]) -> Vec<Token> {
    let mut reader = BitReader::new(bytes.to_vec());
    let (tokens, _offsets) = parse_tokens_impl(&mut reader, true);
    tokens
}

/// Parse raw bytes as a token stream, skipping the 7-bit magic header.
/// Useful for re-parsing embedded data (e.g. String token content).
pub fn parse_raw_tokens(bytes: &[u8]) -> Vec<Token> {
    let mut reader = BitReader::new(bytes.to_vec());
    let mut tokens = Vec::new();

    for _ in 0..200 {
        let Some(prefix2) = reader.read_bits(2) else {
            break;
        };

        match prefix2 {
            0b00 => {
                tokens.push(Token::Separator);
                if reader.remaining_bits() < 8 {
                    break;
                }
            }
            0b01 => {
                tokens.push(Token::SoftSeparator);
            }
            0b10 | 0b11 => {
                let Some(bit3) = reader.read_bits(1) else {
                    break;
                };
                let prefix3 = (prefix2 << 1) | bit3;
                let Some(prefix) = TokenPrefix::from_bits(prefix3) else {
                    break;
                };
                match parse_3bit_token(&mut reader, prefix, false) {
                    Some(t) => tokens.push(t),
                    None if prefix == TokenPrefix::Part => break,
                    None => {}
                }
            }
            _ => break,
        }
    }

    tokens
}

/// Parse an embedded token stream from the original serial bitstream.
///
/// Given the bit offset of a `111` token, skips the 3-bit prefix, reads the
/// VarInt count, then parses tokens from the bitstream at that exact position.
/// Returns (count, tokens, bits_consumed).
pub fn parse_embedded_from_bitstream(
    raw_bytes: &[u8],
    string_token_offset: usize,
) -> (u64, Vec<Token>, usize) {
    let mut reader = BitReader::new(raw_bytes.to_vec());
    reader.bit_offset = string_token_offset + 3; // skip 111 prefix

    let count = reader.read_varint().unwrap_or(0);
    let start = reader.bit_offset;

    let mut tokens = Vec::new();
    for _ in 0..200 {
        let Some(prefix2) = reader.read_bits(2) else {
            break;
        };

        match prefix2 {
            0b00 => {
                tokens.push(Token::Separator);
                if reader.remaining_bits() < 8 {
                    break;
                }
            }
            0b01 => {
                tokens.push(Token::SoftSeparator);
            }
            0b10 | 0b11 => {
                let Some(bit3) = reader.read_bits(1) else {
                    break;
                };
                let prefix3 = (prefix2 << 1) | bit3;
                let Some(prefix) = TokenPrefix::from_bits(prefix3) else {
                    break;
                };
                match parse_3bit_token(&mut reader, prefix, false) {
                    Some(t) => tokens.push(t),
                    None if prefix == TokenPrefix::Part => break,
                    None => {}
                }
            }
            _ => break,
        }
    }

    let bits_consumed = reader.bit_offset - start;
    (count, tokens, bits_consumed)
}

/// Intermediate result for header extraction
struct HeaderInfo {
    manufacturer: Option<u64>,
    level: Option<u64>,
    raw_level: Option<u64>,
    seed: Option<u64>,
}

/// Decode serial prefix, validate, and return raw bytes
fn decode_serial_bytes(serial: &str) -> Result<Vec<u8>, SerialError> {
    if !serial.starts_with("@Ug") {
        return Err(SerialError::InvalidEncoding(
            "Serial must start with @Ug".to_string(),
        ));
    }

    if serial.len() < 5 {
        return Err(SerialError::InvalidEncoding("Serial too short".to_string()));
    }

    let encoded_data = &serial[2..];
    let decoded = decode_base85(encoded_data)?;
    let raw_bytes: Vec<u8> = decoded.iter().map(|&b| mirror_byte(b)).collect();

    if raw_bytes.len() < 4 {
        return Err(SerialError::TooShort {
            expected: 4,
            actual: raw_bytes.len(),
        });
    }

    Ok(raw_bytes)
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
///
/// Header structure: VarBit(category), [SoftSep, VarInt]*, Separator
/// Level is the last VarInt before the first Separator.
fn extract_equipment_header(tokens: &[Token]) -> HeaderInfo {
    let header_varints: Vec<u64> = tokens
        .iter()
        .take_while(|t| !matches!(t, Token::Separator))
        .filter_map(|t| {
            if let Token::VarInt(v) = t {
                Some(*v)
            } else {
                None
            }
        })
        .collect();

    let (level, raw_level) = header_varints
        .last()
        .and_then(|&code| level_from_code(code))
        .map(|(capped, raw)| (Some(capped as u64), Some(raw as u64)))
        .unwrap_or((None, None));

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

/// Extract elements from Part tokens by resolving part names
fn extract_elements(tokens: &[Token], category: i64) -> Vec<Element> {
    tokens
        .iter()
        .filter_map(|token| {
            if let Token::Part { index, .. } = token {
                let name = resolve_part_name(category, *index)?;
                Element::from_part_name(name)
            } else {
                None
            }
        })
        .collect()
}

/// Extract rarity based on item format
fn extract_rarity(tokens: &[Token], is_varbit_first: bool) -> Option<Rarity> {
    if is_varbit_first {
        let first_varbit = tokens.iter().find_map(|t| {
            if let Token::VarBit(v) = t {
                Some(*v)
            } else {
                None
            }
        })?;
        let divisor = varbit_divisor(first_varbit);
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

/// Resolve a part index to a name, trying per-category first then shared verticals.
pub(crate) fn resolve_part_name(category: i64, index: u64) -> Option<&'static str> {
    // Try per-category lookup first (works for all item types)
    if let Some(name) = crate::manifest::part_name(category, index as i64) {
        return Some(name);
    }

    // Only fall back to shared verticals if index is ABOVE per-category range.
    // Indices within the per-category range that don't exist there are ambiguous —
    // multiple shared categories can claim the same index with different parts.
    let max = crate::manifest::max_part_index(category).unwrap_or(0);
    if (index as i64) <= max {
        return None;
    }

    // Fall back to shared vertical categories
    for &shared_cat in SHARED_VERTICAL_CATEGORIES {
        if shared_cat == category {
            continue;
        }
        if let Some(name) = crate::manifest::part_name(shared_cat, index as i64) {
            return Some(name);
        }
    }

    None
}

impl ItemSerial {
    /// Decode a Borderlands 4 item serial
    ///
    /// Format: `@Ug<base85_data>`
    /// Example: @Ugr$ZCm/&tH!t{KgK/Shxu>k
    pub fn decode(serial: &str) -> Result<Self, SerialError> {
        let raw_bytes = decode_serial_bytes(serial)?;

        let mut reader = BitReader::new(raw_bytes.clone());
        let (tokens, token_bit_offsets) = parse_tokens(&mut reader);

        let format = if matches!(tokens.first(), Some(Token::VarBit(_))) {
            SerialFormat::VarBitFirst
        } else {
            SerialFormat::VarIntFirst
        };

        let header = if format == SerialFormat::VarBitFirst {
            extract_equipment_header(&tokens)
        } else {
            extract_weapon_header(&tokens)
        };

        let category = if format == SerialFormat::VarBitFirst {
            tokens
                .iter()
                .find_map(|t| {
                    if let Token::VarBit(v) = t {
                        Some(category_from_varbit(*v))
                    } else {
                        None
                    }
                })
                .unwrap_or(-1)
        } else {
            header
                .manufacturer
                .map(|id| serial_id_to_parts_category(id) as i64)
                .unwrap_or(-1)
        };
        let elements = extract_elements(&tokens, category);
        let rarity = extract_rarity(&tokens, format == SerialFormat::VarBitFirst);

        Ok(ItemSerial {
            original: serial.to_string(),
            raw_bytes,
            format,
            tokens,
            token_bit_offsets,
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

        // Build final serial with prefix (@U; the rest is encoded in the base85 payload)
        format!("@U{}", encoded)
    }

    /// Create a new ItemSerial with modified tokens
    pub fn with_tokens(&self, tokens: Vec<Token>) -> Self {
        let category = self.parts_category().unwrap_or(-1);
        let elements = extract_elements(&tokens, category);

        ItemSerial {
            original: self.original.clone(),
            raw_bytes: self.raw_bytes.clone(), // Will be stale but that's OK
            format: self.format,
            token_bit_offsets: Vec::new(), // Offsets not meaningful for modified tokens
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

    /// Get the NCS category name for this item (e.g., "Jakobs Sniper", "Armor Shield")
    ///
    /// Returns None if the category can't be determined or has no known name.
    pub fn category_name(&self) -> Option<&'static str> {
        self.parts_category()
            .and_then(crate::manifest::category_name)
    }

    /// Get the item type description — category name with "Unknown" fallback
    pub fn item_type_description(&self) -> &'static str {
        self.category_name().unwrap_or("Unknown")
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
        let is_varint_first = matches!(self.tokens.first(), Some(Token::VarInt(_)));
        if is_varint_first {
            self.manufacturer.and_then(weapon_info_from_first_varint)
        } else {
            None
        }
    }

    /// Extract Part Group ID (category) from the serial
    ///
    /// For VarBit-first items, determines the correct divisor from VarBit magnitude
    /// and extracts the NCS category ID. Returns None for VarInt-first items.
    pub fn part_group_id(&self) -> Option<i64> {
        let first_varbit = self.tokens.iter().find_map(|t| {
            if let Token::VarBit(v) = t {
                Some(*v)
            } else {
                None
            }
        })?;

        if !matches!(self.tokens.first(), Some(Token::VarBit(_))) {
            return None;
        }

        Some(category_from_varbit(first_varbit))
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
    /// Returns (index, name, values) tuples where:
    /// - index is the raw part index from the serial
    /// - name is the part name from the manifest (or None if not found)
    /// - values are any associated values
    ///
    /// Lookup order:
    /// 1. Per-category parts (the item's own category)
    /// 2. Shared verticals (stat mods, rarity, barrel/grip/etc. pools)
    /// 3. Base shared parts (category 1)
    pub fn parts_with_names(&self) -> Vec<(u64, Option<&'static str>, Vec<u64>)> {
        let category = self.parts_category().unwrap_or(-1);
        self.parts()
            .into_iter()
            .map(|(index, values)| {
                let name = resolve_part_name(category, index);
                (index, name, values)
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
        for (index, name, values) in parts {
            if name.is_some_and(|n| Element::from_part_name(n).is_some()) {
                continue;
            }

            let part_str = match name {
                Some(n) => {
                    // Extract just the part name after the prefix (e.g., "part_barrel_01" from "DAD_PS.part_barrel_01")
                    let short_name = n.split('.').next_back().unwrap_or(n);
                    if values.is_empty() {
                        short_name.to_string()
                    } else if values.len() == 1 {
                        format!("{}:{}", short_name, values[0])
                    } else {
                        format!("{}:{:?}", short_name, values)
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

    /// Resolve all parts to display-ready structs with names, slots, and element flags.
    ///
    /// Skips zero-index parts. For each part:
    /// - Element markers (128-142) get slot "element" and the element name
    /// - Named parts get their manifest name and slot from `manifest::slot_from_part_name`
    /// - Unknown parts get slot "unknown" and display as "[index]"
    pub fn resolved_parts(&self) -> Vec<ResolvedPart> {
        let parts = self.parts_with_names();
        let mut resolved = Vec::new();

        for (index, name, _values) in parts {
            if index == 0 {
                continue;
            }

            if let Some(n) = name {
                if let Some(element) = Element::from_part_name(n) {
                    resolved.push(ResolvedPart {
                        index,
                        name: Some(n),
                        short_name: element.name().to_string(),
                        slot: "element",
                        is_element: true,
                    });
                    continue;
                }
            }
            if let Some(n) = name {
                let short_name = n.split('.').next_back().unwrap_or(n);
                let slot = crate::manifest::slot_from_part_name(n);
                resolved.push(ResolvedPart {
                    index,
                    name: Some(n),
                    short_name: short_name.to_string(),
                    slot,
                    is_element: false,
                });
            } else {
                resolved.push(ResolvedPart {
                    index,
                    name: None,
                    short_name: format!("[{}]", index),
                    slot: "unknown",
                    is_element: false,
                });
            }
        }

        resolved
    }

    /// Extract all string tokens (UE asset paths) from the token stream.
    pub fn string_tokens(&self) -> Vec<ResolvedString> {
        self.tokens
            .iter()
            .filter_map(|t| {
                if let Token::String(s) = t {
                    let short_name = s.split('.').next_back().unwrap_or(s).to_string();
                    Some(ResolvedString {
                        asset_path: s.clone(),
                        short_name,
                    })
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
        output.push_str(&format!(
            "Format: {} ({})\n",
            self.format,
            self.item_type_description()
        ));
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
            let bit = self.token_bit_offsets.get(i).copied();
            if let Some(b) = bit {
                output.push_str(&format!("  [{:2}@{:3}] {:?}\n", i, b, token));
            } else {
                output.push_str(&format!("  [{:2}    ] {:?}\n", i, token));
            }
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

        assert_eq!(item.format, SerialFormat::VarBitFirst);
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

        assert_eq!(item.format, SerialFormat::VarBitFirst);
        assert!(!item.raw_bytes.is_empty());
        assert_eq!(item.raw_bytes[0], 0x21); // Magic header
    }

    #[test]
    fn test_decode_utility_serial() {
        // VarInt-first item with first VarInt(16) (Vladof Sniper)
        let serial = "@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1";
        let item = ItemSerial::decode(serial).unwrap();

        assert_eq!(item.format, SerialFormat::VarIntFirst);
        assert!(!item.tokens.is_empty());
        assert_eq!(item.manufacturer, Some(16));
    }

    #[test]
    fn test_invalid_serial_prefix() {
        let result = ItemSerial::decode("InvalidSerial");
        assert!(result.is_err());
    }

    #[test]
    fn test_part_group_id_extraction() {
        // Vladof Repair Kit (category 269, VarBit-first)
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        assert_eq!(item.part_group_id(), Some(269));

        // Shield (category 278, VarBit-first)
        let item = ItemSerial::decode("@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_").unwrap();
        assert_eq!(item.part_group_id(), Some(278));

        // VarInt-first items don't use Part Group ID
        let item =
            ItemSerial::decode("@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1").unwrap();
        assert_eq!(item.part_group_id(), None);
    }

    #[test]
    fn test_parts_extraction() {
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let parts = item.parts();
        assert!(!parts.is_empty(), "Should have at least one part");

        // First part has index 1 (with corrected nibble reversal)
        let (index, _values) = &parts[0];
        assert_eq!(*index, 1u64);
    }

    #[test]
    fn test_equipment_level_extraction() {
        // Shield type-e: level 50
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        assert_eq!(item.level, Some(50));

        // Grenade: level 50
        let item = ItemSerial::decode("@Uge8Xtm/)}}!elF;NmXinbwH6?9}OPi1ON").unwrap();
        assert_eq!(item.level, Some(50));

        // Class mod: level 49
        let item = ItemSerial::decode("@Uge8;)m/)@{!X>!SqTZJibf`hSk4B2r6#)").unwrap();
        assert_eq!(item.level, Some(49));

        // Shield type-r: level 30
        let item = ItemSerial::decode("@Ugr$)Nm/%P$!bIqxL{(~iG&p36L=sIx00").unwrap();
        assert_eq!(item.level, Some(30));

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
        // Hellwalker (Fire shotgun) - element is intrinsic to the legendary part,
        // not encoded as a separate element Part token in the serial
        let item = ItemSerial::decode("@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-").unwrap();
        assert_eq!(item.elements.len(), 0);
        assert_eq!(item.element_names(), None);
    }

    #[test]
    fn test_element_extraction_corrosive() {
        // Jakobs Pistol (Shalashaska) - no separate element Part token
        let item = ItemSerial::decode("@UgbV{rFjEj=bZ<~-RG}KRs7TF2b*c{P7OEuz").unwrap();
        assert_eq!(item.elements.len(), 0);
        assert_eq!(item.element_names(), None);
    }

    #[test]
    fn test_element_extraction_none() {
        // Energy Shield - no weapon element
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        assert!(item.elements.is_empty());
        assert_eq!(item.element_names(), None);
    }

    #[test]
    fn test_shalashaska_scope_not_element() {
        // Reported bug: index 26 in Jakobs Pistol is part_scope_01_lens_01,
        // not Corrosive. The old from_index hardcoded map was wrong.
        let item =
            ItemSerial::decode("@UgbV{rFme!K<aW?mRG/*lsIsVasB@@vs7=*D^+EkX%/f+A00}").unwrap();
        assert!(item.elements.is_empty(), "Shalashaska has no element parts");
        let parts = item.resolved_parts();
        let scope = parts.iter().find(|p| p.index == 26).unwrap();
        assert_eq!(scope.short_name, "part_scope_01_lens_01");
        assert_eq!(scope.slot, "scope");
        assert!(!scope.is_element);
    }

    #[test]
    fn test_from_part_name() {
        assert_eq!(Element::from_part_name("part_fire"), Some(Element::Fire));
        assert_eq!(Element::from_part_name("part_cryo"), Some(Element::Cryo));
        assert_eq!(
            Element::from_part_name("part_corrosive"),
            Some(Element::Corrosive)
        );
        assert_eq!(Element::from_part_name("part_shock"), Some(Element::Shock));
        assert_eq!(
            Element::from_part_name("part_radiation"),
            Some(Element::Radiation)
        );
        assert_eq!(
            Element::from_part_name("part_kinetic"),
            Some(Element::Kinetic)
        );
        assert_eq!(Element::from_part_name("part_scope_01_lens_01"), None);
        assert_eq!(Element::from_part_name("part_grip_01"), None);
        assert_eq!(
            Element::from_part_name("part_body_ele_rainbowvomit_cor_fire_shock"),
            Some(Element::Corrosive),
        );
    }

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
        fn test_from_equipment_varbit() {
            // With correct bit ordering, rarity is not encoded in the VarBit
            // (VarBit = category ID directly). Returns None pending re-derivation.
            assert_eq!(Rarity::from_equipment_varbit(279, 1), None);
            assert_eq!(Rarity::from_equipment_varbit(0, 0), None);
        }

        #[test]
        fn test_from_weapon_level_code_common() {
            // With VarInt nibble reversal, level codes are just levels.
            // Rarity is not encoded in the level code for weapons.
            assert_eq!(Rarity::from_weapon_level_code(30), None);
            assert_eq!(Rarity::from_weapon_level_code(50), None);
        }

        #[test]
        fn test_from_weapon_level_code_epic() {
            assert_eq!(Rarity::from_weapon_level_code(48), None);
        }

        #[test]
        fn test_from_weapon_level_code_legendary() {
            assert_eq!(Rarity::from_weapon_level_code(49), None);
        }

        #[test]
        fn test_from_weapon_level_code_ranges() {
            assert_eq!(Rarity::from_weapon_level_code(1), None);
            assert_eq!(Rarity::from_weapon_level_code(50), None);
        }
    }

    #[test]
    #[ignore] // Run with: cargo test -p bl4 level_code_analysis -- --ignored --nocapture
    fn level_code_analysis() {
        use std::collections::BTreeMap;
        use std::fs;

        let serials_path = "/tmp/bl4-all-serials.txt";
        let content = fs::read_to_string(serials_path)
            .expect("Export first: sqlite3 share/items.db 'SELECT serial FROM items' > /tmp/bl4-all-serials.txt");

        let serials: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

        // Collect raw VarInt[3] values for VarInt-first weapons
        let mut varint_codes: BTreeMap<u64, usize> = BTreeMap::new();
        // Collect raw VarBit remainder values for VarBit-first items
        let mut varbit_remainders: BTreeMap<u64, usize> = BTreeMap::new();
        let mut no_level = 0;

        for s in &serials {
            if let Ok(item) = ItemSerial::decode(s) {
                match item.tokens.first() {
                    Some(Token::VarInt(_)) => {
                        // VarInt-first: level code is header VarInt[3]
                        let header: Vec<u64> = item
                            .tokens
                            .iter()
                            .take_while(|t| !matches!(t, Token::Separator))
                            .filter_map(|t| {
                                if let Token::VarInt(v) = t {
                                    Some(*v)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if header.len() >= 4 {
                            *varint_codes.entry(header[3]).or_default() += 1;
                        } else {
                            no_level += 1;
                        }
                    }
                    Some(Token::VarBit(v)) => {
                        let divisor = crate::parts::varbit_divisor(*v);
                        let remainder = v % divisor;
                        *varbit_remainders.entry(remainder).or_default() += 1;
                    }
                    _ => {
                        no_level += 1;
                    }
                }
            }
        }

        println!("\n=== VarInt-first: raw level codes (VarInt[3]) ===");
        let mut sorted: Vec<_> = varint_codes.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (code, count) in &sorted {
            let decoded = crate::parts::level_from_code(**code);
            let odd_marker = if **code >= 128 && (**code - 120) % 2 == 1 {
                " *** ODD ***"
            } else {
                ""
            };
            println!(
                "  code {:>3} (0x{:02x}) = {:>4}x  decoded={:?}{}",
                code, code, count, decoded, odd_marker
            );
        }

        println!("\n=== VarBit-first: remainder values (varbit % divisor) ===");
        let mut sorted: Vec<_> = varbit_remainders.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (remainder, count) in &sorted {
            let rarity_bits = (**remainder >> 6) & 0x3;
            let low_bits = **remainder & 0x3F;
            println!(
                "  remainder {:>3} (0x{:02x}) = {:>4}x  rarity_bits={} low_6_bits={} (0x{:02x})",
                remainder, remainder, count, rarity_bits, low_bits, low_bits
            );
        }

        println!("\n=== Dead zone codes (51-127) detail ===");
        for s in &serials {
            if let Ok(item) = ItemSerial::decode(s) {
                if !matches!(item.tokens.first(), Some(Token::VarInt(_))) {
                    continue;
                }
                let header: Vec<u64> = item
                    .tokens
                    .iter()
                    .take_while(|t| !matches!(t, Token::Separator))
                    .filter_map(|t| {
                        if let Token::VarInt(v) = t {
                            Some(*v)
                        } else {
                            None
                        }
                    })
                    .collect();
                if header.len() >= 4 && (51..128).contains(&header[3]) {
                    let mfr_id = header[0];
                    let mfr = crate::parts::weapon_info_from_first_varint(mfr_id);
                    let rarity = item.rarity;
                    println!(
                        "  code={} mfr_id={} mfr={:?} rarity={:?} header={:?}",
                        header[3], mfr_id, mfr, rarity, header
                    );
                }
            }
        }

        println!("\n=== High codes (>145, non-standard rarity) detail ===");
        for s in &serials {
            if let Ok(item) = ItemSerial::decode(s) {
                if !matches!(item.tokens.first(), Some(Token::VarInt(_))) {
                    continue;
                }
                let header: Vec<u64> = item
                    .tokens
                    .iter()
                    .take_while(|t| !matches!(t, Token::Separator))
                    .filter_map(|t| {
                        if let Token::VarInt(v) = t {
                            Some(*v)
                        } else {
                            None
                        }
                    })
                    .collect();
                if header.len() >= 4 && header[3] > 200 {
                    let mfr_id = header[0];
                    let mfr = crate::parts::weapon_info_from_first_varint(mfr_id);
                    println!(
                        "  code={} (0x{:x}) mfr={:?} header={:?}",
                        header[3], header[3], mfr, header
                    );
                }
            }
        }

        println!("\n=== VarInt[2] distribution ===");
        let mut vi2: BTreeMap<u64, usize> = BTreeMap::new();
        for s in &serials {
            if let Ok(item) = ItemSerial::decode(s) {
                if !matches!(item.tokens.first(), Some(Token::VarInt(_))) {
                    continue;
                }
                let header: Vec<u64> = item
                    .tokens
                    .iter()
                    .take_while(|t| !matches!(t, Token::Separator))
                    .filter_map(|t| {
                        if let Token::VarInt(v) = t {
                            Some(*v)
                        } else {
                            None
                        }
                    })
                    .collect();
                if header.len() >= 3 {
                    *vi2.entry(header[2]).or_default() += 1;
                }
            }
        }
        let mut sorted: Vec<_> = vi2.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (val, count) in &sorted {
            println!("  VarInt[2]={} = {}x", val, count);
        }

        println!("\n=== VarInt[2]=8: VarInt[3] distribution ===");
        let mut vi3_when_8: BTreeMap<u64, usize> = BTreeMap::new();
        let mut vi3_when_4: BTreeMap<u64, usize> = BTreeMap::new();
        for s in &serials {
            if let Ok(item) = ItemSerial::decode(s) {
                if !matches!(item.tokens.first(), Some(Token::VarInt(_))) {
                    continue;
                }
                let header: Vec<u64> = item
                    .tokens
                    .iter()
                    .take_while(|t| !matches!(t, Token::Separator))
                    .filter_map(|t| {
                        if let Token::VarInt(v) = t {
                            Some(*v)
                        } else {
                            None
                        }
                    })
                    .collect();
                if header.len() >= 4 {
                    if header[2] == 8 {
                        *vi3_when_8.entry(header[3]).or_default() += 1;
                    } else if header[2] == 4 {
                        *vi3_when_4.entry(header[3]).or_default() += 1;
                    }
                }
            }
        }
        let mut sorted: Vec<_> = vi3_when_8.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (val, count) in &sorted {
            let decoded = crate::parts::level_from_code(**val);
            println!("  [2]=8, [3]={} = {}x  decoded={:?}", val, count, decoded);
        }
        println!("\n=== VarInt[2]=4: VarInt[3] distribution ===");
        let mut sorted: Vec<_> = vi3_when_4.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (val, count) in &sorted {
            let decoded = crate::parts::level_from_code(**val);
            println!("  [2]=4, [3]={} = {}x  decoded={:?}", val, count, decoded);
        }

        println!("\n=== VarInt[2]=4 item serials ===");
        for s in &serials {
            if let Ok(item) = ItemSerial::decode(s) {
                if !matches!(item.tokens.first(), Some(Token::VarInt(_))) {
                    continue;
                }
                let header: Vec<u64> = item
                    .tokens
                    .iter()
                    .take_while(|t| !matches!(t, Token::Separator))
                    .filter_map(|t| {
                        if let Token::VarInt(v) = t {
                            Some(*v)
                        } else {
                            None
                        }
                    })
                    .collect();
                if header.len() >= 3 && header[2] == 4 {
                    println!("  serial: {}", s);
                    println!("    header: {:?}", header);
                    println!("    all tokens: {:?}", item.tokens);
                    println!();
                }
            }
        }

        println!("\nNo level data: {}", no_level);
    }

    #[test]
    #[ignore] // Run with: cargo test -p bl4 statistical_analysis -- --ignored --nocapture
    fn statistical_analysis() {
        use std::collections::BTreeMap;
        use std::fs;

        let serials_path = "/tmp/bl4-all-serials.txt";
        let content = fs::read_to_string(serials_path)
            .expect("Export serials first: sqlite3 share/items.db 'SELECT serial FROM items' > /tmp/bl4-all-serials.txt");

        let serials: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        println!("\n=== SERIAL STATISTICAL ANALYSIS ===");
        println!("Total serials: {}\n", serials.len());

        let mut decoded: Vec<ItemSerial> = Vec::new();
        let mut failures = 0;
        for s in &serials {
            match ItemSerial::decode(s) {
                Ok(item) => decoded.push(item),
                Err(_) => failures += 1,
            }
        }
        println!("Decoded: {}, Failed: {}\n", decoded.len(), failures);

        // Group by category (using parts_category for both formats)
        let mut by_cat: BTreeMap<i64, Vec<&ItemSerial>> = BTreeMap::new();
        for item in &decoded {
            let cat = item.parts_category().unwrap_or_else(|| {
                // Fallback: use manufacturer ID for VarInt-first
                item.manufacturer.map(|m| m as i64).unwrap_or(-1)
            });
            by_cat.entry(cat).or_default().push(item);
        }

        // For top categories: show per-index value distributions
        println!("=== PART INDEX VALUE ANALYSIS (top 15 categories) ===");
        let mut sorted_cats: Vec<_> = by_cat.iter().collect();
        sorted_cats.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (cat, items) in sorted_cats.iter().take(15) {
            let cat_name = crate::parts::category_name(**cat).unwrap_or("?");
            let n = items.len();
            println!("\n  Category {} ({}, n={}):", cat, cat_name, n);

            // Collect: index → list of value vectors
            let mut index_values: BTreeMap<u64, Vec<&Vec<u64>>> = BTreeMap::new();
            for item in items.iter() {
                for token in &item.tokens {
                    if let Token::Part { index, values } = token {
                        index_values.entry(*index).or_default().push(values);
                    }
                }
            }

            // Show each index: frequency, value patterns
            for (idx, all_values) in &index_values {
                let name = crate::manifest::part_name(**cat, *idx as i64).unwrap_or("?");
                let freq = all_values.len();
                let pct = freq * 100 / n;

                // Analyze value patterns
                let empty_count = all_values.iter().filter(|v| v.is_empty()).count();
                let single_count = all_values.iter().filter(|v| v.len() == 1).count();
                let multi_count = all_values.iter().filter(|v| v.len() > 1).count();

                // For single-value parts, show value distribution
                let value_info = if single_count > 0 {
                    let mut val_dist: BTreeMap<u64, usize> = BTreeMap::new();
                    for v in all_values.iter().filter(|v| v.len() == 1) {
                        *val_dist.entry(v[0]).or_default() += 1;
                    }
                    let mut sorted_vals: Vec<_> = val_dist.iter().collect();
                    sorted_vals.sort_by(|a, b| b.1.cmp(a.1));
                    let top: Vec<String> = sorted_vals
                        .iter()
                        .take(5)
                        .map(|(v, c)| format!("{}x{}", c, v))
                        .collect();
                    format!(" vals=[{}]", top.join(","))
                } else {
                    String::new()
                };

                let multi_info = if multi_count > 0 {
                    // Show sample multi-value
                    let sample = all_values.iter().find(|v| v.len() > 1).unwrap();
                    format!(" multi(len={})={:?}", sample.len(), sample)
                } else {
                    String::new()
                };

                println!(
                    "    idx {:>3} ({:<35}) {:>3}/{} ({:>2}%) empty={} single={} multi={}{}{}",
                    idx,
                    name,
                    freq,
                    n,
                    pct,
                    empty_count,
                    single_count,
                    multi_count,
                    value_info,
                    multi_info
                );
            }
        }

        // String token analysis for VarBit-first items
        println!("\n=== STRING TOKENS IN VARBIT-FIRST ITEMS ===");
        let mut string_values: BTreeMap<String, usize> = BTreeMap::new();
        for item in &decoded {
            if !matches!(item.tokens.first(), Some(Token::VarBit(_))) {
                continue;
            }
            for token in &item.tokens {
                if let Token::String(s) = token {
                    *string_values.entry(s.clone()).or_default() += 1;
                }
            }
        }
        let mut sorted_strings: Vec<_> = string_values.iter().collect();
        sorted_strings.sort_by(|a, b| b.1.cmp(a.1));
        for (s, count) in sorted_strings.iter().take(30) {
            println!("  {:>4}x  {:?}", count, s);
        }
        if sorted_strings.len() > 30 {
            println!(
                "  ... and {} more unique strings",
                sorted_strings.len() - 30
            );
        }

        // Token structure fingerprints
        println!("\n=== TOKEN STRUCTURE FINGERPRINTS (top 15) ===");
        let mut fingerprints: BTreeMap<String, usize> = BTreeMap::new();
        for item in &decoded {
            let fp: String = item
                .tokens
                .iter()
                .map(|t| match t {
                    Token::Separator => "|",
                    Token::SoftSeparator => ".",
                    Token::VarInt(_) => "I",
                    Token::VarBit(_) => "B",
                    Token::Part { .. } => "P",
                    Token::String(_) => "S",
                })
                .collect();
            *fingerprints.entry(fp).or_default() += 1;
        }
        let mut fp_sorted: Vec<_> = fingerprints.iter().collect();
        fp_sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (fp, count) in fp_sorted.iter().take(15) {
            println!("  {:>4}x  {}", count, fp);
        }
    }

    mod display_tests {
        use super::*;

        #[test]
        fn test_hex_dump() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let hex = item.hex_dump();

            assert!(!hex.is_empty());
            assert!(hex.len() % 2 == 0);
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        }

        #[test]
        fn test_format_tokens_weapon() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let formatted = item.format_tokens();

            assert!(!formatted.is_empty());
            assert!(formatted.contains('|') || formatted.contains('{'));
        }

        #[test]
        fn test_format_tokens_structure() {
            let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
            let formatted = item.format_tokens();

            assert!(!formatted.is_empty());
        }

        #[test]
        fn test_item_type_description_weapon() {
            let item = ItemSerial::decode("@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-").unwrap();
            assert_eq!(item.format, SerialFormat::VarIntFirst);
            assert_eq!(item.item_type_description(), "Jakobs Shotgun");
        }

        #[test]
        fn test_item_type_description_varbit_first() {
            let item = ItemSerial::decode("@Ugr$N8m/)}}!q9r4K/ShxuK@").unwrap();
            assert_eq!(item.format, SerialFormat::VarBitFirst);
            assert_eq!(item.item_type_description(), "Torgue Repair Kit");

            let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
            assert_eq!(item.format, SerialFormat::VarBitFirst);
            assert_eq!(item.item_type_description(), "Vladof Enhancement");
        }

        #[test]
        fn test_item_type_description_varint_first() {
            let item = ItemSerial::decode("@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-").unwrap();
            assert_eq!(item.format, SerialFormat::VarIntFirst);
            assert_eq!(item.item_type_description(), "Jakobs Shotgun");

            // VarInt-first with ID not in WEAPON_INFO still resolves via parts_category
            let item =
                ItemSerial::decode("@Ug!pHG2}TYgjMfjzn~K!T)XUVX)U4Eu)Qi+?RPAVZh!@!b00").unwrap();
            assert_eq!(item.format, SerialFormat::VarIntFirst);
            // Category name from NCS, or "Unknown" if unmapped
            let desc = item.item_type_description();
            assert!(!desc.is_empty());
        }

        #[test]
        fn test_manufacturer_name_known() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            if let Some(name) = item.manufacturer_name() {
                assert!(!name.is_empty());
            }
        }

        #[test]
        fn test_rarity_name_method() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            if let Some(rarity) = item.rarity_name() {
                let valid = ["Common", "Uncommon", "Rare", "Epic", "Legendary"];
                assert!(valid.contains(&rarity));
            }
        }

        #[test]
        fn test_weapon_info_for_weapon() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            if let Some((mfg, wtype)) = item.weapon_info() {
                assert!(!mfg.is_empty());
                assert!(!wtype.is_empty());
            }
        }

        #[test]
        fn test_weapon_info_for_equipment() {
            let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
            let _ = item.weapon_info();
        }

        #[test]
        fn test_parts_category() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let category = item.parts_category();
            assert!(category.is_some());
        }

        #[test]
        fn test_detailed_dump() {
            let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
            let dump = item.detailed_dump();

            assert!(dump.contains("Serial:"));
            assert!(dump.contains("Format:"));
            assert!(dump.contains("Bytes:"));
            assert!(dump.contains("Tokens:"));
            assert!(dump.contains("Raw bytes:"));
        }
    }

    #[test]
    #[ignore]
    fn debug_bit_trace() {
        let serial = "@Ugr$ZCm/&tH!t{KgK/Shxu>k"; // VLA_SMG
        let item = ItemSerial::decode(serial).unwrap();
        eprintln!(
            "raw_bytes: {:?}",
            item.raw_bytes
                .iter()
                .map(|b| format!("{:08b}", b))
                .collect::<Vec<_>>()
        );
        for (i, token) in item.tokens.iter().enumerate() {
            let offset = item.token_bit_offsets.get(i).copied().unwrap_or(0);
            eprintln!("  [{:2} @ bit {:3}] {:?}", i, offset, token);
        }
    }

    #[test]
    #[ignore]
    fn debug_token_dump() {
        let serials = [
            ("VLA_SMG", "@Ugr$ZCm/&tH!t{KgK/Shxu>k"),
            ("Shield", "@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_"),
            (
                "VLA_Sniper",
                "@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1",
            ),
            ("Hellwalker", "@Ugd_t@FmVuJyjIXzRG}JG7S$K^1{DjH5&-"),
            ("JAK_Pistol", "@UgbV{rFjEj=bZ<~-RG}KRs7TF2b*c{P7OEuz"),
            ("Shield_e", "@Uge98>m/)}}!c5JeNWCvCXc7"),
            ("Grenade", "@Uge8Xtm/)}}!elF;NmXinbwH6?9}OPi1ON"),
            ("ClassMod", "@Uge8;)m/)@{!X>!SqTZJibf`hSk4B2r6#)"),
            ("Shield_r", "@Ugr$)Nm/%P$!bIqxL{(~iG&p36L=sIx00"),
            ("Weapon_L30", "@Ugb)KvFg_4rJ}%H-RG}IbsZG^E#X_Y-00"),
            (
                "BOR_SMG",
                "@UgxFw!2}TYgOs)+YRG}7?s3AisQ8!UBQ8Q6BQDIPXP<2qdQ2P)",
            ),
            ("Equipment272", "@Uge8aum/(OZ$pj+I_5#Y(pw{;WbgA{xWRhC/"),
            ("Equipment321", "@Ugr%Scm/)}}$pj({qzigfrP>z<v^$y<L5*r(1po"),
            ("Grenade2", "@Ugr$N8m/)}}!q9r4K/ShxuK@"),
        ];
        for (name, serial) in &serials {
            let item = ItemSerial::decode(serial).unwrap();
            eprintln!("\n=== {} ===", name);
            eprintln!("  format: {:?}", item.format);
            eprintln!("  tokens: {:?}", item.tokens);
            eprintln!("  manufacturer: {:?}", item.manufacturer);
            eprintln!("  level: {:?}, raw_level: {:?}", item.level, item.raw_level);
            eprintln!("  rarity: {:?}", item.rarity);
            eprintln!("  elements: {:?}", item.elements);
            eprintln!("  part_group_id: {:?}", item.part_group_id());
            eprintln!("  parts_category: {:?}", item.parts_category());
        }
    }
}
