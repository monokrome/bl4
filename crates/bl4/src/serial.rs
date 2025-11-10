//! Item serial number decoding for Borderlands 4
//!
//! Item serials use a custom Base85 encoding with bit-packed data.
//!
//! Format:
//! 1. Serials start with `@U` prefix
//! 2. Encoded with custom Base85 alphabet
//! 3. Decoded bytes have mirrored bits
//! 4. Data is a variable-length bitstream with tokens
//!
//! Based on: https://github.com/Nicnl/borderlands4-serials

/// Custom Base85 alphabet used by Borderlands 4
const BL4_BASE85_ALPHABET: &[u8; 85] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~";

/// Errors that can occur during serial decoding
#[derive(Debug, thiserror::Error)]
pub enum SerialError {
    #[error("Invalid ASCII85 encoding: {0}")]
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

impl BitReader {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    /// Read N bits as a u64 value
    /// Bits are read MSB-first from left to right
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

    /// Read a VARINT (4-bit nibbles with continuation bit)
    fn read_varint(&mut self) -> Option<u64> {
        let mut result = 0u64;
        let mut shift = 0;

        // Max 16 bits = 4 nibbles
        for _ in 0..4 {
            let nibble = self.read_bits(4)?;
            let value = nibble & 0x7; // Lower 3 bits are data
            let cont = (nibble & 0x8) != 0; // High bit is continuation

            result |= value << shift;
            shift += 3;

            if !cont {
                return Some(result);
            }
        }

        Some(result)
    }

    /// Read a VARBIT (5-bit length prefix + variable data)
    fn read_varbit(&mut self) -> Option<u64> {
        let length = self.read_bits(5)? as usize;
        self.read_bits(length)
    }

    #[allow(dead_code)]
    fn current_bit_offset(&self) -> usize {
        self.bit_offset
    }
}

/// Token types in the bitstream
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Separator,           // 00
    SoftSeparator,       // 01
    VarInt(u64),         // 100 + varint data
    VarBit(u64),         // 110 + varbit data
    Part { index: u64, values: Vec<u64> },  // 101 + part data
    String(String),      // 111 + length + ascii
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
    pub manufacturer: Option<u64>,
    pub rarity: Option<u64>,
    pub level: Option<u64>,
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
fn mirror_byte(byte: u8) -> u8 {
    let mut result = 0u8;
    for i in 0..8 {
        if byte & (1 << i) != 0 {
            result |= 1 << (7 - i);
        }
    }
    result
}

/// Parse tokens from bitstream
fn parse_tokens(reader: &mut BitReader) -> Vec<Token> {
    let mut tokens = Vec::new();

    // Verify magic header (7 bits = 0010000)
    if let Some(magic) = reader.read_bits(7) {
        if magic != 0b0010000 {
            eprintln!("Warning: Invalid magic header: {:07b}", magic);
            return tokens;
        }
    } else {
        return tokens;
    }

    // Parse tokens until terminator (00)
    loop {
        // Read token type (2-3 bits)
        let prefix2 = match reader.read_bits(2) {
            Some(p) => p,
            None => break,
        };

        match prefix2 {
            0b00 => {
                tokens.push(Token::Separator);
                break; // Terminator
            }
            0b01 => {
                tokens.push(Token::SoftSeparator);
            }
            0b10 | 0b11 => {
                // Need one more bit to distinguish
                let bit3 = match reader.read_bits(1) {
                    Some(b) => b,
                    None => break,
                };

                let prefix3 = (prefix2 << 1) | bit3;

                match prefix3 {
                    0b100 => {
                        // VARINT
                        if let Some(val) = reader.read_varint() {
                            tokens.push(Token::VarInt(val));
                        }
                    }
                    0b110 => {
                        // VARBIT
                        if let Some(val) = reader.read_varbit() {
                            tokens.push(Token::VarBit(val));
                        }
                    }
                    0b101 => {
                        // Part structure
                        if let Some(index) = reader.read_varint() {
                            let mut values = Vec::new();

                            // Check if single value or multiple
                            if let Some(flag) = reader.read_bits(1) {
                                if flag == 0 {
                                    // Single value
                                    if let Some(val) = reader.read_varint() {
                                        values.push(val);
                                    }
                                } else {
                                    // Multiple values
                                    if let Some(count) = reader.read_varint() {
                                        for _ in 0..count {
                                            if let Some(val) = reader.read_varint() {
                                                values.push(val);
                                            }
                                        }
                                    }
                                }
                            }

                            tokens.push(Token::Part { index, values });
                        }
                    }
                    0b111 => {
                        // String
                        if let Some(length) = reader.read_bits(8) {
                            let mut chars = Vec::new();
                            for _ in 0..length {
                                if let Some(ch) = reader.read_bits(8) {
                                    chars.push(ch as u8);
                                }
                            }
                            if let Ok(s) = String::from_utf8(chars) {
                                tokens.push(Token::String(s));
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

        // Extract common fields from tokens (basic heuristic)
        let mut manufacturer = None;
        let mut rarity = None;
        let mut level = None;

        // First few VarInts are typically manufacturer, rarity, level
        let varints: Vec<u64> = tokens
            .iter()
            .filter_map(|t| {
                if let Token::VarInt(v) = t {
                    Some(*v)
                } else {
                    None
                }
            })
            .collect();

        if varints.len() > 0 {
            manufacturer = Some(varints[0]);
        }
        if varints.len() > 1 {
            rarity = Some(varints[1]);
        }
        if varints.len() > 2 {
            level = Some(varints[2]);
        }

        Ok(ItemSerial {
            original: serial.to_string(),
            raw_bytes,
            item_type,
            tokens,
            manufacturer,
            rarity,
            level,
        })
    }

    /// Display hex dump of raw bytes
    pub fn hex_dump(&self) -> String {
        hex::encode(&self.raw_bytes)
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
        if let Some(r) = self.rarity {
            output.push_str(&format!("  Rarity: {}\n", r));
        }
        if let Some(l) = self.level {
            output.push_str(&format!("  Level: {}\n", l));
        }
        output.push_str("\n");

        // Show parsed tokens
        output.push_str(&format!("Tokens: {} total\n", self.tokens.len()));
        for (i, token) in self.tokens.iter().enumerate() {
            output.push_str(&format!("  [{:2}] {:?}\n", i, token));
        }
        output.push_str("\n");

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
        assert!(item.raw_bytes.len() > 0);
        assert!(item.tokens.len() > 0, "Should parse at least one token");

        // First byte should be 0x21 (contains magic header 0010000)
        assert_eq!(item.raw_bytes[0], 0x21);
    }

    #[test]
    fn test_decode_equipment_serial() {
        // Equipment serial from save file
        let serial = "@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_";
        let item = ItemSerial::decode(serial).unwrap();

        assert_eq!(item.item_type, 'e');
        assert!(item.raw_bytes.len() > 0);
        assert_eq!(item.raw_bytes[0], 0x21); // Magic header
    }

    #[test]
    fn test_decode_utility_serial() {
        // Utility item with VarInt manufacturer
        let serial = "@Uguq~c2}TYg3/>%aRG}8ts7KXA-9&{!<w2c7r9#z0g+sMN<wF1";
        let item = ItemSerial::decode(serial).unwrap();

        assert_eq!(item.item_type, 'u');
        assert!(item.tokens.len() > 0);

        // Should have manufacturer extracted
        assert_eq!(item.manufacturer, Some(0));
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
}
