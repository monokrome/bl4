//! Texture parsing and BC7 decoding for UE5 textures
//!
//! Parses FTexturePlatformData from cooked texture assets and decodes
//! BC7 compressed textures to RGBA.

use anyhow::{Result, bail, Context};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Read, Cursor, Seek, SeekFrom};
use std::path::Path;

/// Pixel formats we support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    BC7,
    BC1,
    BC3,
    BC4,
    BC5,
    RGBA8,
    Unknown,
}

impl PixelFormat {
    pub fn from_name(name: &str) -> Self {
        match name {
            "PF_BC7" => PixelFormat::BC7,
            "PF_DXT1" => PixelFormat::BC1,
            "PF_DXT5" => PixelFormat::BC3,
            "PF_BC4" => PixelFormat::BC4,
            "PF_BC5" => PixelFormat::BC5,
            "PF_B8G8R8A8" | "PF_R8G8B8A8" => PixelFormat::RGBA8,
            _ => PixelFormat::Unknown,
        }
    }

    /// Bytes per block for block-compressed formats, or bytes per pixel
    pub fn bytes_per_block(&self) -> usize {
        match self {
            PixelFormat::BC1 | PixelFormat::BC4 => 8,
            PixelFormat::BC7 | PixelFormat::BC3 | PixelFormat::BC5 => 16,
            PixelFormat::RGBA8 => 4,
            PixelFormat::Unknown => 0,
        }
    }

    /// Block size in pixels (4x4 for BC formats, 1x1 for uncompressed)
    pub fn block_size(&self) -> usize {
        match self {
            PixelFormat::BC1 | PixelFormat::BC3 | PixelFormat::BC4 |
            PixelFormat::BC5 | PixelFormat::BC7 => 4,
            PixelFormat::RGBA8 => 1,
            PixelFormat::Unknown => 1,
        }
    }
}

/// A single mip level
#[derive(Debug)]
pub struct TextureMip {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub data_size: u64,
    pub data_offset: u64,  // Offset within ubulk file
}

/// Parsed texture metadata from .uasset
#[derive(Debug)]
pub struct TextureInfo {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub format_name: String,
    pub num_slices: u32,
    pub mips: Vec<TextureMip>,
    pub ubulk_offset: u64,  // Total offset to first mip in ubulk
}

/// Parse texture metadata from the cooked serial data within a .uasset
///
/// This parses FTexturePlatformData structure which contains:
/// - SizeX, SizeY (dimensions)
/// - PackedData (num slices, flags)
/// - PixelFormat (string)
/// - Mip chain with bulk data references
pub fn parse_texture_info(data: &[u8], header_size: usize) -> Result<TextureInfo> {
    // The texture data is in the export's cooked serial data
    // We need to skip the Zen header and find the FTexturePlatformData

    // For UE5 textures, the structure after the standard UObject properties is:
    // - FStripDataFlags (4 bytes)
    // - bCooked bool (1 byte, usually 1)
    // - FTexturePlatformData follows

    let mut cursor = Cursor::new(data);

    // Skip to the export data (past the Zen package header)
    // The header_size tells us where the export serial data begins
    cursor.seek(SeekFrom::Start(header_size as u64))?;

    // Now we're in the export data. For Texture2D, we need to find:
    // 1. Skip UObject serialized properties (varies)
    // 2. Find FTexturePlatformData

    // Let's search for the pixel format string as an anchor
    let search_data = &data[header_size..];
    let format_offset = find_pixel_format(search_data)?;

    // Back up to find SizeX, SizeY before the format
    // FTexturePlatformData layout (UE5):
    // - int32 SizeX
    // - int32 SizeY
    // - uint32 PackedData
    // - FString PixelFormat (length-prefixed)
    // - int32 FirstMipToSerialize
    // - int32 NumMips
    // - FTexture2DMipMap[] Mips

    // The format_offset points to the length prefix of the format string
    // Go back 12 bytes to find SizeX
    if format_offset < 12 {
        bail!("Not enough data before pixel format for dimensions");
    }

    let platform_data_start = header_size + format_offset - 12;
    cursor.seek(SeekFrom::Start(platform_data_start as u64))?;

    let size_x = cursor.read_u32::<LittleEndian>()?;
    let size_y = cursor.read_u32::<LittleEndian>()?;
    let packed_data = cursor.read_u32::<LittleEndian>()?;

    // Read format string (FString: length + chars)
    let format_len = cursor.read_u32::<LittleEndian>()? as usize;
    if format_len > 64 {
        bail!("Invalid format string length: {}", format_len);
    }
    let mut format_bytes = vec![0u8; format_len];
    cursor.read_exact(&mut format_bytes)?;
    // Remove null terminator if present
    let format_name = String::from_utf8_lossy(&format_bytes)
        .trim_end_matches('\0')
        .to_string();

    let format = PixelFormat::from_name(&format_name);

    // Extract num_slices from packed_data (lower bits)
    let num_slices = packed_data & 0x0FFF;

    // Read mip info
    // Skip optional data based on packed flags
    let has_opt_data = (packed_data & 0x40000000) != 0;
    if has_opt_data {
        // Skip FOptTexturePlatformData (ExtData + NumMipsInTail)
        cursor.seek(SeekFrom::Current(8))?;
    }

    let first_mip = cursor.read_i32::<LittleEndian>()?;
    let num_mips = cursor.read_i32::<LittleEndian>()? as usize;

    if num_mips > 20 {
        bail!("Too many mips: {}", num_mips);
    }

    let mut mips = Vec::with_capacity(num_mips);
    let mut _current_offset: u64 = 0;

    for _i in 0..num_mips {
        // FTexture2DMipMap:
        // - FByteBulkData (complex, has flags, size, offset)
        // - int32 SizeX
        // - int32 SizeY
        // - int32 SizeZ (UE4.20+)

        // FByteBulkData structure (simplified for cooked data):
        // - uint32 BulkDataFlags
        // - int64 ElementCount (data size in bytes for textures)
        // - int64 BulkDataOffsetInFile

        let bulk_flags = cursor.read_u32::<LittleEndian>()?;
        let element_count = cursor.read_i64::<LittleEndian>()? as u64;
        let bulk_offset = cursor.read_i64::<LittleEndian>()? as u64;

        let mip_width = cursor.read_u32::<LittleEndian>()?;
        let mip_height = cursor.read_u32::<LittleEndian>()?;
        let mip_depth = cursor.read_u32::<LittleEndian>()?;

        // If data is inline (not in ubulk), we'd need different handling
        // For cooked textures, it's usually in .ubulk

        mips.push(TextureMip {
            width: mip_width,
            height: mip_height,
            depth: mip_depth.max(1),
            data_size: element_count,
            data_offset: bulk_offset,
        });

        _current_offset += element_count;
    }

    Ok(TextureInfo {
        width: size_x,
        height: size_y,
        format,
        format_name,
        num_slices,
        mips,
        ubulk_offset: 0, // Will be set from bulk data offset
    })
}

/// Find the pixel format string in the data
fn find_pixel_format(data: &[u8]) -> Result<usize> {
    // Look for common format strings
    // All patterns must be same length - pad with 0s
    // Using 12-byte arrays to accommodate longest format names
    let patterns: &[&[u8]] = &[
        b"PF_BC7\0",
        b"PF_DXT1\0",
        b"PF_DXT5\0",
        b"PF_BC4\0",
        b"PF_BC5\0",
        b"PF_B8G8R8A8\0",
    ];

    for pattern in patterns {
        // Search for the format string (length-prefixed)
        // Length is stored as u32 before the string
        let pattern_len = pattern.len() as u32;
        let len_bytes = pattern_len.to_le_bytes();

        for i in 0..data.len().saturating_sub(pattern.len() + 4) {
            if data[i..i+4] == len_bytes &&
               i + 4 + pattern.len() <= data.len() &&
               &data[i+4..i+4+pattern.len()] == *pattern {
                return Ok(i);
            }
        }
    }

    bail!("Could not find pixel format in texture data");
}

/// Convert u32 RGBA buffer to u8 RGBA buffer
fn u32_to_u8_rgba(u32_buf: &[u32]) -> Vec<u8> {
    let mut result = Vec::with_capacity(u32_buf.len() * 4);
    for &pixel in u32_buf {
        // texture2ddecoder uses ARGB or BGRA layout, need to convert to RGBA
        let b = (pixel & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let r = ((pixel >> 16) & 0xFF) as u8;
        let a = ((pixel >> 24) & 0xFF) as u8;
        result.push(r);
        result.push(g);
        result.push(b);
        result.push(a);
    }
    result
}

/// Decode a BC7 texture to RGBA
pub fn decode_bc7(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;

    // BC7 uses 4x4 blocks, 16 bytes per block
    let blocks_x = (w + 3) / 4;
    let blocks_y = (h + 3) / 4;
    let expected_size = blocks_x * blocks_y * 16;

    if data.len() < expected_size {
        bail!("BC7 data too small: got {}, expected {}", data.len(), expected_size);
    }

    let mut output = vec![0u32; w * h];

    texture2ddecoder::decode_bc7(data, w, h, &mut output)
        .map_err(|e| anyhow::anyhow!("BC7 decode failed: {:?}", e))?;

    Ok(u32_to_u8_rgba(&output))
}

/// Decode BC1 (DXT1) texture to RGBA
pub fn decode_bc1(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;
    let mut output = vec![0u32; w * h];

    texture2ddecoder::decode_bc1(data, w, h, &mut output)
        .map_err(|e| anyhow::anyhow!("BC1 decode failed: {:?}", e))?;

    Ok(u32_to_u8_rgba(&output))
}

/// Decode BC3 (DXT5) texture to RGBA
pub fn decode_bc3(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;
    let mut output = vec![0u32; w * h];

    texture2ddecoder::decode_bc3(data, w, h, &mut output)
        .map_err(|e| anyhow::anyhow!("BC3 decode failed: {:?}", e))?;

    Ok(u32_to_u8_rgba(&output))
}

/// Save RGBA data as PNG
pub fn save_png(rgba_data: &[u8], width: u32, height: u32, path: &Path) -> Result<()> {
    use image::{ImageBuffer, Rgba};

    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, rgba_data.to_vec())
        .context("Failed to create image buffer")?;

    img.save(path).context("Failed to save PNG")?;

    Ok(())
}

/// Extract texture from .uasset and .ubulk files
pub fn extract_texture(
    uasset_data: &[u8],
    ubulk_data: &[u8],
    header_size: usize,
    output_path: &Path,
    mip_level: usize,  // 0 = highest resolution
) -> Result<()> {
    let info = parse_texture_info(uasset_data, header_size)?;

    if info.mips.is_empty() {
        bail!("No mip levels in texture");
    }

    let mip_idx = mip_level.min(info.mips.len() - 1);
    let mip = &info.mips[mip_idx];

    // Read mip data from ubulk
    let data_start = mip.data_offset as usize;
    let data_end = data_start + mip.data_size as usize;

    if data_end > ubulk_data.len() {
        bail!(
            "Mip data out of bounds: offset {} + size {} > ubulk size {}",
            data_start, mip.data_size, ubulk_data.len()
        );
    }

    let mip_data = &ubulk_data[data_start..data_end];

    // Decode based on format
    let rgba = match info.format {
        PixelFormat::BC7 => decode_bc7(mip_data, mip.width, mip.height)?,
        PixelFormat::BC1 => decode_bc1(mip_data, mip.width, mip.height)?,
        PixelFormat::BC3 => decode_bc3(mip_data, mip.width, mip.height)?,
        PixelFormat::RGBA8 => mip_data.to_vec(),
        _ => bail!("Unsupported pixel format: {:?}", info.format),
    };

    // Save as PNG
    save_png(&rgba, mip.width, mip.height, output_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_format_parsing() {
        assert_eq!(PixelFormat::from_name("PF_BC7"), PixelFormat::BC7);
        assert_eq!(PixelFormat::from_name("PF_DXT1"), PixelFormat::BC1);
        assert_eq!(PixelFormat::from_name("PF_DXT5"), PixelFormat::BC3);
        assert_eq!(PixelFormat::from_name("unknown"), PixelFormat::Unknown);
    }
}
