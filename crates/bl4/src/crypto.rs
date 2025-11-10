//! Borderlands 4 save file encryption and decryption
//!
//! Based on work from glacierpiece:
//! https://github.com/glacierpiece/borderlands-4-save-utlity

#[allow(deprecated)]
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes256;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Base encryption key used for all Borderlands 4 save files
const BASE_KEY: [u8; 32] = [
    0x35, 0xEC, 0x33, 0x77, 0xF3, 0x5D, 0xB0, 0xEA, 0xBE, 0x6B, 0x83, 0x11, 0x54, 0x03, 0xEB, 0xFB,
    0x27, 0x25, 0x64, 0x2E, 0xD5, 0x49, 0x06, 0x29, 0x05, 0x78, 0xBD, 0x60, 0xBA, 0x4A, 0xA7, 0x87,
];

/// Errors that can occur during encryption/decryption
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Save file size {0} is not a multiple of 16 bytes")]
    InvalidSize(usize),

    #[error("Failed to decompress YAML data: {0}")]
    DecompressionError(#[from] std::io::Error),

    #[error("Invalid padding in encrypted data")]
    InvalidPadding,

    #[error("Invalid Steam ID format")]
    InvalidSteamId,
}

/// Apply PKCS7 padding to data
fn pkcs7_pad(data: &[u8], block_size: usize) -> Vec<u8> {
    let padding_len = block_size - (data.len() % block_size);
    let mut padded = Vec::with_capacity(data.len() + padding_len);
    padded.extend_from_slice(data);
    padded.extend(std::iter::repeat_n(padding_len as u8, padding_len));
    padded
}

/// Remove PKCS7 padding from data
fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.is_empty() {
        return Err(CryptoError::InvalidPadding);
    }

    let padding_len = *data.last().unwrap() as usize;

    if padding_len == 0 || padding_len > data.len() {
        return Err(CryptoError::InvalidPadding);
    }

    // Verify all padding bytes are correct
    for &byte in &data[data.len() - padding_len..] {
        if byte as usize != padding_len {
            return Err(CryptoError::InvalidPadding);
        }
    }

    Ok(data[..data.len() - padding_len].to_vec())
}

/// Derive an AES-256 key from a Steam ID
///
/// The key is derived by XORing the first 8 bytes of BASE_KEY with
/// the Steam ID encoded as an 8-byte little-endian integer.
pub fn derive_key(steam_id: &str) -> Result<[u8; 32], CryptoError> {
    // Extract digits from Steam ID and parse as u64
    let digits: String = steam_id.chars().filter(|c| c.is_ascii_digit()).collect();
    let steam_id_num = digits
        .parse::<u64>()
        .map_err(|_| CryptoError::InvalidSteamId)?;

    // Convert to 8-byte little-endian
    let steam_id_bytes = steam_id_num.to_le_bytes();

    // XOR first 8 bytes of BASE_KEY with Steam ID bytes
    let mut key = BASE_KEY;
    for i in 0..8 {
        key[i] ^= steam_id_bytes[i];
    }

    Ok(key)
}

/// Decrypt a .sav file to YAML bytes
///
/// # Format
/// - Input: AES-256-ECB encrypted, PKCS7 padded
/// - After decryption: zlib compressed YAML data
pub fn decrypt_sav(encrypted_data: &[u8], steam_id: &str) -> Result<Vec<u8>, CryptoError> {
    // Validate input size (must be multiple of 16 for AES block cipher)
    if !encrypted_data.len().is_multiple_of(16) {
        return Err(CryptoError::InvalidSize(encrypted_data.len()));
    }

    // Derive key from Steam ID
    let key = derive_key(steam_id)?;
    #[allow(deprecated)]
    let cipher = Aes256::new(GenericArray::from_slice(&key));

    // Decrypt using AES-256-ECB (process 16-byte blocks)
    let mut decrypted = encrypted_data.to_vec();
    for chunk in decrypted.chunks_exact_mut(16) {
        #[allow(deprecated)]
        cipher.decrypt_block(GenericArray::from_mut_slice(chunk));
    }

    // Try to remove PKCS7 padding, but fall back to using padded data
    // (Python code does this - some saves may not use standard padding)
    let unpadded = pkcs7_unpad(&decrypted).unwrap_or_else(|_| decrypted.clone());

    // Decompress zlib data
    let mut decoder = ZlibDecoder::new(&unpadded[..]);
    let mut yaml_data = Vec::new();
    decoder.read_to_end(&mut yaml_data)?;

    Ok(yaml_data)
}

/// Encrypt YAML bytes to a .sav file
///
/// # Format
/// - Compresses YAML with zlib (level 9)
/// - Pads with PKCS7 to 16-byte blocks
/// - Encrypts with AES-256-ECB
pub fn encrypt_sav(yaml_data: &[u8], steam_id: &str) -> Result<Vec<u8>, CryptoError> {
    // Compress with zlib
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(yaml_data)?;
    let mut compressed = encoder.finish()?;

    // Append footer: adler32 checksum (4 bytes) + uncompressed length (4 bytes)
    let adler32 = adler::adler32_slice(yaml_data);
    let uncompressed_len = yaml_data.len() as u32;

    compressed.extend_from_slice(&adler32.to_le_bytes());
    compressed.extend_from_slice(&uncompressed_len.to_le_bytes());

    // Pad to 16-byte blocks
    let mut encrypted = pkcs7_pad(&compressed, 16);

    // Derive key and encrypt
    let key = derive_key(steam_id)?;
    #[allow(deprecated)]
    let cipher = Aes256::new(GenericArray::from_slice(&key));

    for chunk in encrypted.chunks_exact_mut(16) {
        #[allow(deprecated)]
        cipher.encrypt_block(GenericArray::from_mut_slice(chunk));
    }

    Ok(encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key() {
        let steam_id = "76561197960521364";
        let key = derive_key(steam_id).unwrap();

        // First 8 bytes should be XORed with Steam ID
        assert_ne!(key[0..8], BASE_KEY[0..8]);
        // Remaining bytes should be unchanged
        assert_eq!(key[8..], BASE_KEY[8..]);
    }

    #[test]
    fn test_roundtrip() {
        let steam_id = "76561197960521364";
        let original_yaml = b"test: value\nfoo: bar\n";

        let encrypted = encrypt_sav(original_yaml, steam_id).unwrap();
        let decrypted = decrypt_sav(&encrypted, steam_id).unwrap();

        assert_eq!(original_yaml, &decrypted[..]);
    }
}
