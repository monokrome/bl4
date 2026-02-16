//! Bit-level data reading for NCS binary parsing
//!
//! Provides utilities for reading packed binary data at the bit level,
//! including variable-length integers and fixed-width arrays.

/// Bitstream reader for parsing packed binary data
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Read n bits as u32
    pub fn read_bits(&mut self, n: u8) -> Option<u32> {
        if n == 0 || n > 32 {
            return None;
        }

        let mut result: u32 = 0;
        let mut bits_read = 0u8;

        while bits_read < n {
            if self.byte_pos >= self.data.len() {
                return None;
            }

            let remaining_in_byte = 8 - self.bit_pos;
            let bits_to_read = remaining_in_byte.min(n - bits_read);

            let mask = ((1u32 << bits_to_read) - 1) as u8;
            let byte_val = self.data[self.byte_pos];
            let extracted = (byte_val >> self.bit_pos) & mask;

            result |= (extracted as u32) << bits_read;
            bits_read += bits_to_read;
            self.bit_pos += bits_to_read;

            if self.bit_pos >= 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }

        Some(result)
    }

    /// Read a single bit
    pub fn read_bit(&mut self) -> Option<bool> {
        self.read_bits(1).map(|v| v != 0)
    }

    /// Read variable-length integer (Elias gamma coding)
    pub fn read_varint(&mut self) -> Option<u32> {
        // Count leading zeros
        let mut zeros = 0u8;
        while !self.read_bit()? {
            zeros += 1;
            if zeros > 30 {
                return None;
            }
        }

        if zeros == 0 {
            return Some(1);
        }

        // Read the value bits
        let value = self.read_bits(zeros)?;
        Some((1 << zeros) | value)
    }

    /// Check if we've reached end of data
    pub fn is_empty(&self) -> bool {
        self.byte_pos >= self.data.len()
    }

    /// Get current position in bits
    pub fn position(&self) -> usize {
        self.byte_pos * 8 + self.bit_pos as usize
    }

    /// Get total bits available
    pub fn total_bits(&self) -> usize {
        self.data.len() * 8
    }

    /// Check if n more bits are available
    pub fn has_bits(&self, n: usize) -> bool {
        self.position() + n <= self.total_bits()
    }

    /// Read a FixedWidthIntArray24 header (24-bit count + 8-bit width)
    /// Returns (count, bit_width)
    pub fn read_fixed_width_header(&mut self) -> Option<(u32, u8)> {
        let count = self.read_bits(24)?;
        let width = self.read_bits(8)? as u8;
        Some((count, width))
    }

    /// Read n entries of `width` bits each
    pub fn read_fixed_width_array(&mut self, count: u32, width: u8) -> Option<Vec<u32>> {
        if width == 0 || width > 32 {
            return Some(Vec::new());
        }
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            values.push(self.read_bits(width)?);
        }
        Some(values)
    }

    /// Skip to byte boundary
    pub fn align_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    /// Get remaining bytes (for debugging)
    pub fn remaining_bytes(&self) -> &[u8] {
        if self.bit_pos == 0 {
            &self.data[self.byte_pos..]
        } else {
            &self.data[self.byte_pos + 1..]
        }
    }

    /// Seek to a specific bit position
    pub fn seek(&mut self, bit_pos: usize) {
        self.byte_pos = bit_pos / 8;
        self.bit_pos = (bit_pos % 8) as u8;
    }

    /// Skip n bits
    pub fn skip_bits(&mut self, n: usize) {
        let new_pos = self.position() + n;
        self.seek(new_pos);
    }
}

/// Calculate minimum bits needed to index a table of `count` entries
pub fn bit_width(count: u32) -> u8 {
    if count < 2 {
        return 1;
    }
    let mut n = count - 1;
    let mut bits = 0u8;
    while n > 0 {
        bits += 1;
        n >>= 1;
    }
    bits.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader_basic() {
        let data = [0b10110101, 0b11001010];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(1), Some(1));
        assert_eq!(reader.read_bits(1), Some(0));
        assert_eq!(reader.read_bits(1), Some(1));
        assert_eq!(reader.read_bits(1), Some(0));
        assert_eq!(reader.read_bits(4), Some(0b1011));
    }

    #[test]
    fn test_bit_reader_cross_byte() {
        let data = [0xFF, 0xFF];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(12), Some(0xFFF));
    }

    #[test]
    fn test_bit_width() {
        assert_eq!(bit_width(0), 1);
        assert_eq!(bit_width(1), 1);
        assert_eq!(bit_width(2), 1);
        assert_eq!(bit_width(3), 2);
        assert_eq!(bit_width(4), 2);
        assert_eq!(bit_width(5), 3);
        assert_eq!(bit_width(256), 8);
    }
}
