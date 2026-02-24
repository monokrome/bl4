//! Decode String tokens with 7-bit character mirroring (per Nicnl's research).
//!
//! Each 7-bit character in a String token has its bits reversed.

use bl4::serial::Token;
use std::fs;
use std::io::{BufRead, BufReader, Write};

/// Mirror a 7-bit value: reverse bit order within 7 bits
fn mirror_7bit(val: u8) -> u8 {
    let mut result = 0u8;
    for i in 0..7 {
        if val & (1 << i) != 0 {
            result |= 1 << (6 - i);
        }
    }
    result
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/string_token_serials.txt".to_string());
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/string_decoded_text.txt".to_string());

    let reader = BufReader::new(fs::File::open(&input)?);
    let mut out = fs::File::create(&output)?;

    let mut total = 0u32;

    for line in reader.lines() {
        let serial = line?;
        let serial = serial.trim();
        if serial.is_empty() {
            continue;
        }

        let Ok(item) = bl4::ItemSerial::decode(serial) else {
            continue;
        };

        for token in &item.tokens {
            if let Token::String(s) = token {
                total += 1;

                // The current parser already reads 7-bit chunks but doesn't mirror.
                // Apply 7-bit mirror to each byte.
                let mirrored: String = s
                    .bytes()
                    .map(|b| mirror_7bit(b) as char)
                    .collect();

                writeln!(
                    out,
                    "{}\t{}",
                    &serial[..serial.len().min(50)],
                    mirrored
                )?;
            }
        }
    }

    eprintln!("Decoded {} strings with 7-bit mirror → {}", total, output);
    Ok(())
}
