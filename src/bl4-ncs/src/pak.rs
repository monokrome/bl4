//! PAK file reading using proper index-based extraction
//!
//! This module uses `repak` to read PAK files properly via their file index,
//! rather than scanning for magic bytes. This ensures all NCS files are found.

use crate::data::decompress;
use crate::Result;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// A trait for reading files from a data source (PAK file, directory, or in-memory)
pub trait NcsReader: Send + Sync {
    /// List all NCS files in this source
    fn list_ncs_files(&self) -> Result<Vec<String>>;

    /// Read raw NCS data by filename
    fn read_ncs(&mut self, filename: &str) -> Result<Vec<u8>>;

    /// Read and decompress NCS data by filename
    fn read_ncs_decompressed(&mut self, filename: &str) -> Result<Vec<u8>> {
        let raw = self.read_ncs(filename)?;
        decompress(&raw)
    }
}

/// Reader for PAK files on disk using repak
pub struct PakReader {
    pak: repak::PakReader,
    file: BufReader<File>,
}

impl PakReader {
    /// Open a PAK file for reading
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref()).map_err(crate::Error::Io)?;
        let mut reader = BufReader::new(file);

        let pak = repak::PakBuilder::new()
            .reader(&mut reader)
            .map_err(|e| crate::Error::Oodle(format!("Failed to open PAK: {}", e)))?;

        Ok(Self { pak, file: reader })
    }

    /// Get the mount point for this PAK
    pub fn mount_point(&self) -> &str {
        self.pak.mount_point()
    }

    /// List all files in the PAK
    pub fn list_all_files(&self) -> Vec<String> {
        self.pak.files()
    }
}

impl NcsReader for PakReader {
    fn list_ncs_files(&self) -> Result<Vec<String>> {
        Ok(self
            .pak
            .files()
            .into_iter()
            .filter(|f| f.ends_with(".ncs"))
            .collect())
    }

    fn read_ncs(&mut self, filename: &str) -> Result<Vec<u8>> {
        self.pak
            .get(filename, &mut self.file)
            .map_err(|e| crate::Error::Oodle(format!("Failed to read {}: {}", filename, e)))
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
            .map_err(|e| crate::Error::Oodle(format!("Failed to parse PAK: {}", e)))?;

        Ok(Self { pak, data: cursor })
    }
}

impl NcsReader for MemoryPakReader {
    fn list_ncs_files(&self) -> Result<Vec<String>> {
        Ok(self
            .pak
            .files()
            .into_iter()
            .filter(|f| f.ends_with(".ncs"))
            .collect())
    }

    fn read_ncs(&mut self, filename: &str) -> Result<Vec<u8>> {
        self.pak
            .get(filename, &mut self.data)
            .map_err(|e| crate::Error::Oodle(format!("Failed to read {}: {}", filename, e)))
    }
}

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
}

impl NcsReader for DirectoryReader {
    fn list_ncs_files(&self) -> Result<Vec<String>> {
        Ok(self.files.clone())
    }

    fn read_ncs(&mut self, filename: &str) -> Result<Vec<u8>> {
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
}

/// Extract type name from NCS filename
/// "Engine/Content/_NCS/Nexus-Data-itempool0.ncs" -> "itempool"
/// "Nexus-Data-itempool0.ncs" -> "itempool"
pub fn type_from_filename(filename: &str) -> String {
    // Get just the filename part (strip directory path)
    let name = filename
        .rsplit('/')
        .next()
        .unwrap_or(filename);

    name.strip_prefix("Nexus-Data-")
        .and_then(|s| {
            // Remove trailing digit + .ncs (e.g., "0.ncs", "6.ncs")
            s.rfind(|c: char| c.is_ascii_digit())
                .map(|pos| &s[..pos])
        })
        .unwrap_or(name)
        .to_string()
}

/// Extracted NCS file metadata and data
#[derive(Debug, Clone)]
pub struct ExtractedNcs {
    /// Original filename from PAK (e.g., "Nexus-Data-itempool0.ncs")
    pub filename: String,
    /// Type name extracted from filename (e.g., "itempool")
    pub type_name: String,
    /// Raw NCS data (compressed)
    pub raw_data: Vec<u8>,
}

impl ExtractedNcs {
    /// Decompress the NCS data
    pub fn decompress(&self) -> Result<Vec<u8>> {
        decompress(&self.raw_data)
    }
}

/// Extract all NCS files from a reader
pub fn extract_all<R: NcsReader>(reader: &mut R) -> Result<Vec<ExtractedNcs>> {
    let files = reader.list_ncs_files()?;
    let mut results = Vec::with_capacity(files.len());

    for filename in files {
        let raw_data = reader.read_ncs(&filename)?;
        let type_name = type_from_filename(&filename);
        results.push(ExtractedNcs {
            filename,
            type_name,
            raw_data,
        });
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
        // Fallback for non-standard names
        assert_eq!(type_from_filename("custom.ncs"), "custom.ncs");
    }

    #[test]
    fn test_pak_reader_real() {
        let pak_path = std::path::Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            println!("PAK file not found, skipping test");
            return;
        }

        let mut reader = PakReader::open(pak_path).expect("open PAK");
        let ncs_files = reader.list_ncs_files().expect("list NCS files");

        println!("Found {} NCS files in PAK index", ncs_files.len());
        for f in ncs_files.iter().take(10) {
            println!("  {}", f);
        }

        // This should find all 170 files, not just 164
        assert!(
            ncs_files.len() >= 170,
            "Expected at least 170 NCS files, found {}",
            ncs_files.len()
        );

        // Try reading one
        let first = &ncs_files[0];
        let data = reader.read_ncs(first).expect("read NCS");
        println!("Read {} bytes from {}", data.len(), first);

        // Try decompressing
        let decompressed = reader.read_ncs_decompressed(first).expect("decompress");
        println!("Decompressed to {} bytes", decompressed.len());
    }

    #[test]
    fn test_extract_all_real() {
        let pak_path = std::path::Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            return;
        }

        let mut reader = PakReader::open(pak_path).expect("open PAK");
        let extracted = extract_all(&mut reader).expect("extract all");

        println!("Extracted {} NCS files", extracted.len());

        // Count unique types
        let types: std::collections::HashSet<_> =
            extracted.iter().map(|e| &e.type_name).collect();
        println!("Unique types: {}", types.len());

        // Show first 10
        for e in extracted.iter().take(10) {
            println!("  {} ({}) - {} bytes", e.filename, e.type_name, e.raw_data.len());
        }
    }

    #[test]
    fn test_previously_missing_types() {
        let pak_path = std::path::Path::new("/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks/pakchunk0-Windows_0_P.pak");

        if !pak_path.exists() {
            println!("PAK not found, skipping");
            return;
        }

        let reader = PakReader::open(pak_path).expect("open PAK");
        let files = reader.list_ncs_files().expect("list");

        // These 6 types were missing with magic byte scanning
        let previously_missing = [
            "wwise_auxilary_busses",
            "wwise_soundbanks",
            "wwise_states",
            "wwise_switches",
            "wwise_triggers",
            "xp_progression",
        ];

        println!("\nChecking previously missing types:");
        for m in &previously_missing {
            let found = files.iter().any(|f| {
                type_from_filename(f).to_lowercase() == *m
            });
            println!("  {}: {}", m, if found { "FOUND ✓" } else { "MISSING ✗" });
            assert!(found, "Type '{}' should be found with proper PAK reading", m);
        }

        println!("\nAll 6 previously missing types are now found!");
    }
}
