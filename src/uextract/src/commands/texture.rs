//! Texture extraction command

use crate::texture;
use anyhow::{Context, Result};
use std::path::Path;

pub struct ExtractTextureOptions<'a> {
    pub ubulk_path: &'a Path,
    pub width: u32,
    pub height: u32,
    pub output_path: &'a Path,
    pub mip_level: usize,
    pub format: &'a str,
}

#[allow(clippy::too_many_lines)]
pub fn extract_texture_cmd(opts: &ExtractTextureOptions<'_>) -> Result<()> {
    let ExtractTextureOptions {
        ubulk_path,
        width,
        height,
        output_path,
        mip_level,
        format,
    } = opts;
    use std::io::Read;

    let bytes_per_block: u64 = match *format {
        "bc1" | "dxt1" => 8,
        "bc7" => 16,
        _ => 16,
    };

    eprintln!("Reading texture: {:?}", ubulk_path);
    eprintln!("Dimensions: {}x{}, format: {}", width, height, format);

    let mut file = std::fs::File::open(ubulk_path).context("Failed to open ubulk file")?;

    let mut mip_width = *width;
    let mut mip_height = *height;
    let mut offset: u64 = 0;

    for i in 0..*mip_level {
        let blocks_x = (mip_width as u64).div_ceil(4);
        let blocks_y = (mip_height as u64).div_ceil(4);
        let mip_size = blocks_x * blocks_y * bytes_per_block;

        offset += mip_size;
        mip_width = (mip_width / 2).max(1);
        mip_height = (mip_height / 2).max(1);

        eprintln!(
            "Skipping mip {}: {}x{} ({} bytes)",
            i,
            mip_width * 2,
            mip_height * 2,
            mip_size
        );
    }

    eprintln!(
        "Extracting mip {}: {}x{} at offset {}",
        mip_level, mip_width, mip_height, offset
    );

    let blocks_x = (mip_width as usize).div_ceil(4);
    let blocks_y = (mip_height as usize).div_ceil(4);
    let mip_size = blocks_x * blocks_y * bytes_per_block as usize;

    // Seek to the mip and read it
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(offset))?;

    let mut mip_data = vec![0u8; mip_size];
    file.read_exact(&mut mip_data)
        .context("Failed to read mip data")?;

    eprintln!("Read {} bytes of {} data", mip_data.len(), format);

    // Decode to RGBA based on format
    let rgba = match *format {
        "bc1" | "dxt1" => texture::decode_bc1(&mip_data, mip_width, mip_height)
            .context("Failed to decode BC1 texture")?,
        "bc7" => texture::decode_bc7(&mip_data, mip_width, mip_height)
            .context("Failed to decode BC7 texture")?,
        _ => anyhow::bail!("Unsupported format: {}", format),
    };

    eprintln!("Decoded to {} bytes of RGBA", rgba.len());

    // Save as PNG
    texture::save_png(&rgba, mip_width, mip_height, output_path).context("Failed to save PNG")?;

    eprintln!("Saved to {:?}", output_path);

    Ok(())
}
