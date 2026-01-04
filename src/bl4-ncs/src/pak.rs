//! NCS file utilities
//!
//! This module provides utilities for working with NCS files from various sources.
//! PAK file reading is handled by `uextract::pak` - this module focuses on NCS-specific logic.

use crate::data::decompress;
use crate::Result;
use std::path::Path;

/// Reader for a directory of extracted NCS files
pub struct DirectoryReader {
    path: std::path::PathBuf,
    files: Vec<String>,
}

impl DirectoryReader {
    /// Open a directory containing NCS files
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut files = Vec::new();

        for entry in walkdir::WalkDir::new(&path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    if ext == "ncs" || ext == "bin" {
                        if let Some(name) = entry_path.file_name() {
                            files.push(name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        Ok(Self { path, files })
    }

    /// List all NCS/bin files in this directory
    pub fn list_files(&self) -> &[String] {
        &self.files
    }

    /// Read a file by name
    pub fn read(&self, filename: &str) -> Result<Vec<u8>> {
        // Try direct path first
        let direct = self.path.join(filename);
        if direct.exists() {
            return std::fs::read(&direct).map_err(crate::Error::Io);
        }

        // Search recursively for the file
        for entry in walkdir::WalkDir::new(&self.path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(name) = entry_path.file_name() {
                    if name.to_string_lossy() == filename {
                        return std::fs::read(entry_path).map_err(crate::Error::Io);
                    }
                }
            }
        }

        Err(crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", filename),
        )))
    }

    /// Read and decompress a file
    pub fn read_decompressed(&self, filename: &str) -> Result<Vec<u8>> {
        let raw = self.read(filename)?;
        decompress(&raw)
    }
}

/// Extract type name from NCS filename
/// "Engine/Content/_NCS/Nexus-Data-itempool0.ncs" -> "itempool"
/// "Nexus-Data-itempool0.ncs" -> "itempool"
pub fn type_from_filename(filename: &str) -> String {
    // Get just the filename part (strip directory path)
    let name = filename.rsplit('/').next().unwrap_or(filename);

    // Also handle Windows paths
    let name = name.rsplit('\\').next().unwrap_or(name);

    name.strip_prefix("Nexus-Data-")
        .and_then(|s| {
            // Remove trailing digit + .ncs (e.g., "0.ncs", "6.ncs")
            s.rfind(|c: char| c.is_ascii_digit())
                .map(|pos| &s[..pos])
        })
        .unwrap_or(name)
        .to_string()
}

/// Check if a filename is an NCS file
pub fn is_ncs_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".ncs")
}

/// Extracted NCS file metadata and data
#[derive(Debug, Clone)]
pub struct ExtractedNcs {
    /// Original filename (e.g., "Nexus-Data-itempool0.ncs")
    pub filename: String,
    /// Type name extracted from filename (e.g., "itempool")
    pub type_name: String,
    /// Raw NCS data (compressed)
    pub raw_data: Vec<u8>,
}

impl ExtractedNcs {
    /// Create from filename and data
    pub fn new(filename: String, raw_data: Vec<u8>) -> Self {
        let type_name = type_from_filename(&filename);
        Self {
            filename,
            type_name,
            raw_data,
        }
    }

    /// Decompress the NCS data
    pub fn decompress(&self) -> Result<Vec<u8>> {
        decompress(&self.raw_data)
    }
}

/// Extract all NCS files from a directory
pub fn extract_from_directory<P: AsRef<Path>>(path: P) -> Result<Vec<ExtractedNcs>> {
    let reader = DirectoryReader::open(path)?;
    let mut results = Vec::with_capacity(reader.files.len());

    for filename in &reader.files {
        let raw_data = reader.read(filename)?;
        results.push(ExtractedNcs::new(filename.clone(), raw_data));
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_from_filename() {
        assert_eq!(type_from_filename("Nexus-Data-itempool0.ncs"), "itempool");
        assert_eq!(
            type_from_filename("Nexus-Data-achievement6.ncs"),
            "achievement"
        );
        assert_eq!(
            type_from_filename("Nexus-Data-aim_assist_parameters0.ncs"),
            "aim_assist_parameters"
        );
        // With path prefix
        assert_eq!(
            type_from_filename("Engine/Content/_NCS/Nexus-Data-itempool0.ncs"),
            "itempool"
        );
        // Windows path
        assert_eq!(
            type_from_filename("Engine\\Content\\_NCS\\Nexus-Data-itempool0.ncs"),
            "itempool"
        );
        // Fallback for non-standard names
        assert_eq!(type_from_filename("custom.ncs"), "custom.ncs");
    }

    #[test]
    fn test_is_ncs_file() {
        assert!(is_ncs_file("test.ncs"));
        assert!(is_ncs_file("Nexus-Data-itempool0.ncs"));
        assert!(is_ncs_file("path/to/file.NCS"));
        assert!(!is_ncs_file("test.bin"));
        assert!(!is_ncs_file("test.txt"));
    }
}
