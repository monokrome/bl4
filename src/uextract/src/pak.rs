//! Traditional PAK file reading using repak
//!
//! This module provides extraction from Unreal Engine .pak files using the repak library.
//! It complements the IoStore (.utoc/.ucas) support provided by retoc.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Reader for traditional PAK files on disk
pub struct PakReader {
    pak: repak::PakReader,
    file: BufReader<File>,
    path: std::path::PathBuf,
}

impl PakReader {
    /// Open a PAK file for reading
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf)
            .with_context(|| format!("Failed to open PAK file: {:?}", path_buf))?;
        let mut reader = BufReader::new(file);

        let pak = repak::PakBuilder::new()
            .reader(&mut reader)
            .with_context(|| format!("Failed to parse PAK file: {:?}", path_buf))?;

        Ok(Self {
            pak,
            file: reader,
            path: path_buf,
        })
    }

    /// Get the mount point for this PAK
    pub fn mount_point(&self) -> &str {
        self.pak.mount_point()
    }

    /// Get the path this PAK was opened from
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// List all files in the PAK
    pub fn files(&self) -> Vec<String> {
        self.pak.files()
    }

    /// List files matching a filter
    pub fn files_matching<F>(&self, filter: F) -> Vec<String>
    where
        F: Fn(&str) -> bool,
    {
        self.pak.files().into_iter().filter(|f| filter(f)).collect()
    }

    /// List files with a specific extension
    pub fn files_with_extension(&self, ext: &str) -> Vec<String> {
        let ext_lower = ext.to_lowercase();
        let with_dot = if ext_lower.starts_with('.') {
            ext_lower
        } else {
            format!(".{}", ext_lower)
        };

        self.files_matching(|f| f.to_lowercase().ends_with(&with_dot))
    }

    /// Read a file from the PAK
    pub fn read(&mut self, filename: &str) -> Result<Vec<u8>> {
        self.pak
            .get(filename, &mut self.file)
            .with_context(|| format!("Failed to read '{}' from PAK", filename))
    }

    /// Read a file, trying with and without mount point prefix
    pub fn read_flexible(&mut self, filename: &str) -> Result<Vec<u8>> {
        // Try exact match first
        if let Ok(data) = self.read(filename) {
            return Ok(data);
        }

        // Copy mount point to avoid borrow issues
        let mount = self.mount_point().to_string();

        // Try with mount point prefix
        if !mount.is_empty() && !filename.starts_with(&mount) {
            let with_mount = format!("{}{}", mount, filename);
            if let Ok(data) = self.read(&with_mount) {
                return Ok(data);
            }
        }

        // Try without mount point prefix
        if !mount.is_empty() && filename.starts_with(&mount) {
            let without_mount = &filename[mount.len()..];
            if let Ok(data) = self.read(without_mount) {
                return Ok(data);
            }
        }

        // Return original error
        self.read(filename)
    }
}

/// Reader for in-memory PAK data
pub struct MemoryPakReader {
    pak: repak::PakReader,
    data: std::io::Cursor<Vec<u8>>,
}

impl MemoryPakReader {
    /// Create a reader from in-memory PAK data
    pub fn new(data: Vec<u8>) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);

        let pak = repak::PakBuilder::new()
            .reader(&mut cursor)
            .context("Failed to parse in-memory PAK data")?;

        Ok(Self { pak, data: cursor })
    }

    /// Get the mount point for this PAK
    pub fn mount_point(&self) -> &str {
        self.pak.mount_point()
    }

    /// List all files in the PAK
    pub fn files(&self) -> Vec<String> {
        self.pak.files()
    }

    /// List files with a specific extension
    pub fn files_with_extension(&self, ext: &str) -> Vec<String> {
        let ext_lower = ext.to_lowercase();
        let with_dot = if ext_lower.starts_with('.') {
            ext_lower
        } else {
            format!(".{}", ext_lower)
        };

        self.pak
            .files()
            .into_iter()
            .filter(|f| f.to_lowercase().ends_with(&with_dot))
            .collect()
    }

    /// Read a file from the PAK
    pub fn read(&mut self, filename: &str) -> Result<Vec<u8>> {
        self.pak
            .get(filename, &mut self.data)
            .with_context(|| format!("Failed to read '{}' from PAK", filename))
    }
}

/// Extract files from a PAK to a directory
pub fn extract_to_directory<P: AsRef<Path>>(
    pak_path: P,
    output_dir: P,
    filter: Option<&dyn Fn(&str) -> bool>,
) -> Result<Vec<String>> {
    let mut reader = PakReader::open(&pak_path)?;
    let output = output_dir.as_ref();

    std::fs::create_dir_all(output)
        .with_context(|| format!("Failed to create output directory: {:?}", output))?;

    let files: Vec<String> = match filter {
        Some(f) => reader.files_matching(f),
        None => reader.files(),
    };

    let mut extracted = Vec::with_capacity(files.len());

    for filename in &files {
        let data = reader.read(filename)?;

        // Clean up the path (remove mount point, normalize)
        let clean_name = filename
            .trim_start_matches(reader.mount_point())
            .trim_start_matches('/')
            .trim_start_matches("../");

        let out_path = output.join(clean_name);

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&out_path, &data)
            .with_context(|| format!("Failed to write {:?}", out_path))?;

        extracted.push(filename.clone());
    }

    Ok(extracted)
}

/// Scan a directory for PAK files
pub fn find_pak_files<P: AsRef<Path>>(dir: P) -> Result<Vec<std::path::PathBuf>> {
    let mut paks = Vec::new();

    for entry in walkdir::WalkDir::new(dir.as_ref())
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("pak") {
                    paks.push(path.to_path_buf());
                }
            }
        }
    }

    paks.sort();
    Ok(paks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_pak() -> Option<std::path::PathBuf> {
        if let Ok(dir) = std::env::var("BL4_PAKS_DIR") {
            let path = std::path::PathBuf::from(dir).join("pakchunk0-Windows_0_P.pak");
            if path.exists() {
                return Some(path);
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(home) = dirs::home_dir() {
                let path = home.join(".local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");
                if path.exists() {
                    return Some(path);
                }
            }
        }

        None
    }

    #[test]
    fn test_pak_reader() {
        let Some(pak_path) = default_pak() else {
            println!("PAK file not found, skipping test");
            return;
        };

        let reader = PakReader::open(&pak_path).expect("open PAK");
        let files = reader.files();

        println!("Found {} files in PAK", files.len());
        println!("Mount point: {}", reader.mount_point());

        // Check for NCS files
        let ncs_files = reader.files_with_extension("ncs");
        println!("Found {} .ncs files", ncs_files.len());
        assert!(ncs_files.len() >= 170, "Expected at least 170 NCS files");
    }
}
