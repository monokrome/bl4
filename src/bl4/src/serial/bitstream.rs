//! Bitstream reader and writer for variable-length token parsing.

/// Bitstream reader for parsing variable-length tokens
pub(crate) struct BitReader {
    bytes: Vec<u8>,
    pub(crate) bit_offset: usize,
}

impl BitReader {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    /// Read N bits as a u64 value (MSB-first)
    /// Bits are read from the stream and assembled with first bit = MSB
    pub fn read_bits(&mut self, count: usize) -> Option<u64> {
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
    pub fn read_varint(&mut self) -> Option<u64> {
        let mut result = 0u64;
        let mut shift = 0;

        for _ in 0..4 {
            let nibble = self.read_bits(4)?;
            result |= nibble << shift;
            shift += 4;

            let cont = self.read_bits(1)?;
            if cont == 0 {
                return Some(result);
            }
        }

        Some(result)
    }

    /// Read a VARBIT (5-bit length prefix + variable data)
    /// Format: [5-bit length][N-bit value]. Length 0 means value is 0.
    pub fn read_varbit(&mut self) -> Option<u64> {
        let length = self.read_bits(5)? as usize;
        self.read_bits(length)
    }

    #[allow(dead_code)]
    pub fn current_bit_offset(&self) -> usize {
        self.bit_offset
    }

    /// Returns the number of bits remaining in the stream
    pub fn remaining_bits(&self) -> usize {
        let total_bits = self.bytes.len() * 8;
        total_bits.saturating_sub(self.bit_offset)
    }
}

/// Bitstream writer for encoding variable-length tokens
pub(crate) struct BitWriter {
    bytes: Vec<u8>,
    bit_offset: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_offset: 0,
        }
    }

    /// Write N bits from a u64 value (MSB-first)
    pub fn write_bits(&mut self, value: u64, count: usize) {
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
    pub fn write_varint(&mut self, value: u64) {
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
    pub fn write_varbit(&mut self, value: u64) {
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
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip() {
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
