//! NCS extraction with manifest correlation
//!
//! Properly correlates NCS chunks with manifest entries to get authoritative
//! type names and ensure we only extract valid NCS data.

use crate::data::{decompress, scan as scan_chunks, Header};
use crate::manifest::{scan as scan_manifests, Entry};
use crate::Result;

/// A fully identified NCS file with manifest metadata
#[derive(Debug, Clone)]
pub struct NcsFile {
    /// Filename from manifest (e.g., "Nexus-Data-itempool0.ncs")
    pub filename: String,
    /// Type name extracted from filename (e.g., "itempool")
    pub type_name: String,
    /// Manifest index value
    pub index: u32,
    /// Offset in pak file
    pub offset: usize,
    /// NCS header
    pub header: Header,
}

impl NcsFile {
    /// Decompress this NCS file from pak data
    pub fn decompress(&self, pak_data: &[u8]) -> Result<Vec<u8>> {
        decompress(&pak_data[self.offset..])
    }

    /// Extract type name from manifest filename
    /// "Nexus-Data-itempool0.ncs" -> "itempool"
    fn type_from_filename(filename: &str) -> String {
        filename
            .strip_prefix("Nexus-Data-")
            .and_then(|s| {
                // Remove trailing digit + .ncs (e.g., "0.ncs", "6.ncs")
                s.rfind(|c: char| c.is_ascii_digit())
                    .map(|pos| &s[..pos])
            })
            .unwrap_or(filename)
            .to_string()
    }
}

/// Result of extracting NCS from a pak file
#[derive(Debug)]
pub struct ExtractionResult {
    /// Successfully correlated NCS files
    pub files: Vec<NcsFile>,
    /// Manifest entries without corresponding chunks
    pub missing_chunks: Vec<Entry>,
    /// Chunks without manifest entries (likely false positives)
    pub orphan_chunks: Vec<(usize, Header)>,
}

/// Extract all NCS files from pak data using manifest correlation
pub fn extract_from_pak(pak_data: &[u8]) -> ExtractionResult {
    // Find manifest(s)
    let manifests = scan_manifests(pak_data);

    if manifests.is_empty() {
        // No manifest = no valid NCS (any chunks found are false positives)
        let orphans = scan_chunks(pak_data);
        return ExtractionResult {
            files: Vec::new(),
            missing_chunks: Vec::new(),
            orphan_chunks: orphans,
        };
    }

    // Combine all manifest entries (usually just one manifest per pak)
    let mut all_entries: Vec<&Entry> = manifests
        .iter()
        .flat_map(|(_, m)| m.entries.iter())
        .collect();

    // Sort entries by index
    all_entries.sort_by_key(|e| e.index);

    // Get NCS chunks sorted by offset
    let mut chunks = scan_chunks(pak_data);
    chunks.sort_by_key(|(offset, _)| *offset);

    // Correlate: entry N corresponds to chunk N
    let mut files = Vec::new();
    let mut missing_chunks = Vec::new();

    for (i, entry) in all_entries.iter().enumerate() {
        if i < chunks.len() {
            let (offset, header) = chunks[i];
            files.push(NcsFile {
                filename: entry.filename.clone(),
                type_name: NcsFile::type_from_filename(&entry.filename),
                index: entry.index,
                offset,
                header,
            });
        } else {
            // Manifest entry without corresponding chunk
            missing_chunks.push((*entry).clone());
        }
    }

    // Any extra chunks beyond manifest entries are orphans
    let orphan_chunks = if chunks.len() > all_entries.len() {
        chunks[all_entries.len()..].to_vec()
    } else {
        Vec::new()
    };

    ExtractionResult {
        files,
        missing_chunks,
        orphan_chunks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NCS_MAGIC, NCS_MANIFEST_MAGIC};

    fn make_ncs_header(compressed: u32) -> Vec<u8> {
        let mut data = vec![0x01]; // version
        data.extend_from_slice(&NCS_MAGIC);
        data.extend_from_slice(&0u32.to_le_bytes()); // compression_flag
        data.extend_from_slice(&compressed.to_le_bytes()); // decompressed
        data.extend_from_slice(&compressed.to_le_bytes()); // compressed
        data
    }

    fn make_manifest_entry(filename: &str, index: u32) -> Vec<u8> {
        let mut data = Vec::new();
        let len = (filename.len() + 1) as u32;
        data.extend_from_slice(&len.to_le_bytes());
        data.extend_from_slice(filename.as_bytes());
        data.push(0);
        data.extend_from_slice(&index.to_le_bytes());
        data
    }

    fn make_manifest(entries: &[(&str, u32)]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&NCS_MANIFEST_MAGIC);
        data.push(0);
        data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        data.extend_from_slice(&[0, 0]); // padding to 10 bytes

        for (filename, index) in entries {
            data.extend_from_slice(&make_manifest_entry(filename, *index));
        }
        data
    }

    #[test]
    fn test_type_from_filename() {
        assert_eq!(
            NcsFile::type_from_filename("Nexus-Data-itempool0.ncs"),
            "itempool"
        );
        assert_eq!(
            NcsFile::type_from_filename("Nexus-Data-achievement6.ncs"),
            "achievement"
        );
        assert_eq!(
            NcsFile::type_from_filename("Nexus-Data-aim_assist_parameters0.ncs"),
            "aim_assist_parameters"
        );
    }

    #[test]
    fn test_extract_no_manifest() {
        // Data with NCS chunk but no manifest
        let mut data = make_ncs_header(8);
        data.extend_from_slice(&[0u8; 8]);

        let result = extract_from_pak(&data);
        assert!(result.files.is_empty());
        assert!(result.missing_chunks.is_empty());
        assert_eq!(result.orphan_chunks.len(), 1); // False positive
    }

    #[test]
    fn test_extract_with_manifest() {
        let mut data = Vec::new();

        // NCS chunk 1
        let chunk1_offset = data.len();
        data.extend_from_slice(&make_ncs_header(4));
        data.extend_from_slice(&[0u8; 4]);

        // NCS chunk 2
        let chunk2_offset = data.len();
        data.extend_from_slice(&make_ncs_header(8));
        data.extend_from_slice(&[0u8; 8]);

        // Manifest
        data.extend_from_slice(&make_manifest(&[
            ("Nexus-Data-weapons0.ncs", 100),
            ("Nexus-Data-shields0.ncs", 112),
        ]));

        let result = extract_from_pak(&data);
        assert_eq!(result.files.len(), 2);
        assert_eq!(result.files[0].filename, "Nexus-Data-weapons0.ncs");
        assert_eq!(result.files[0].type_name, "weapons");
        assert_eq!(result.files[0].offset, chunk1_offset);
        assert_eq!(result.files[1].filename, "Nexus-Data-shields0.ncs");
        assert_eq!(result.files[1].type_name, "shields");
        assert_eq!(result.files[1].offset, chunk2_offset);
        assert!(result.missing_chunks.is_empty());
        assert!(result.orphan_chunks.is_empty());
    }

    #[test]
    fn test_extract_missing_chunk() {
        let mut data = Vec::new();

        // Only 1 NCS chunk
        data.extend_from_slice(&make_ncs_header(4));
        data.extend_from_slice(&[0u8; 4]);

        // Manifest with 2 entries
        data.extend_from_slice(&make_manifest(&[
            ("Nexus-Data-weapons0.ncs", 100),
            ("Nexus-Data-shields0.ncs", 112),
        ]));

        let result = extract_from_pak(&data);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.missing_chunks.len(), 1);
        assert_eq!(
            result.missing_chunks[0].filename,
            "Nexus-Data-shields0.ncs"
        );
    }
}
