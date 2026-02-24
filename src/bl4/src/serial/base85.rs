//! Base85 encoding/decoding with custom Borderlands 4 alphabet.

use super::SerialError;

/// Custom Base85 alphabet used by Borderlands 4
pub(crate) const BL4_BASE85_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~";

/// Mirror bits in a byte (reverse bit order)
/// Example: 0b10000111 -> 0b11100001
#[inline]
pub fn mirror_byte(byte: u8) -> u8 {
    byte.reverse_bits()
}

/// Decode Base85 with custom BL4 alphabet
pub fn decode_base85(input: &str) -> Result<Vec<u8>, SerialError> {
    // Strip backslashes — shells (zsh/bash) insert `\!` for `!` even in single
    // quotes; `\` is not in the base85 alphabet so any occurrence is pollution.
    let cleaned: String;
    let input = if input.contains('\\') {
        cleaned = input.replace('\\', "");
        &cleaned
    } else {
        input
    };

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

/// Encode bytes to Base85 with custom BL4 alphabet
pub fn encode_base85(bytes: &[u8]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mirror_byte() {
        assert_eq!(mirror_byte(0b10000000), 0b00000001);
        assert_eq!(mirror_byte(0b11000000), 0b00000011);
        assert_eq!(mirror_byte(0b10101010), 0b01010101);
        assert_eq!(mirror_byte(0b00000000), 0b00000000);
        assert_eq!(mirror_byte(0b11111111), 0b11111111);
    }

    #[test]
    fn test_base85_decode() {
        // Test basic Base85 decoding
        let result = decode_base85("g").unwrap();
        assert_eq!(result.len(), 0); // Single char decodes to 0 bytes with partial chunk
    }
}
