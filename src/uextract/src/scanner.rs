//! IoStore scanning for targeted asset extraction
//!
//! Provides `IoStoreScanner` — a wrapper around retoc's IoStore that consolidates
//! the boilerplate for opening stores, loading schemas, and scanning for specific
//! asset classes.

use anyhow::{Context, Result};
use rayon::prelude::*;
use retoc::{
    container_header::EIoContainerHeaderVersion,
    iostore::{self, IoStoreTrait},
    script_objects::FPackageObjectIndexType,
    zen::FZenPackageHeader,
    AesKey, Config, EIoStoreTocVersion, FGuid,
};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use usmap::Usmap;

use crate::types::ZenAssetInfo;
use crate::zen::parse_zen_asset;

/// Raw export data from an asset, for custom deserialization.
pub struct RawExportData {
    pub path: String,
    pub package_name: String,
    pub name_map: Vec<String>,
    pub exports: Vec<RawExport>,
}

/// A single raw export within an asset.
pub struct RawExport {
    pub index: usize,
    pub name: String,
    pub data: Vec<u8>,
}

/// Extract matching raw exports from a parsed Zen package header.
fn extract_raw_exports(
    header: &FZenPackageHeader,
    data: &[u8],
    header_end: usize,
    target_hash: &str,
) -> Vec<RawExport> {
    header
        .export_map
        .iter()
        .enumerate()
        .filter_map(|(ei, export)| {
            if export.class_index.kind() != FPackageObjectIndexType::ScriptImport {
                return None;
            }
            if format!("{:X}", export.class_index.raw_index()) != target_hash {
                return None;
            }

            let obj_name = header.name_map.get(export.object_name).to_string();
            let offset = header_end + export.cooked_serial_offset as usize;
            let size = export.cooked_serial_size as usize;

            if offset + size > data.len() {
                return None;
            }

            Some(RawExport {
                index: ei,
                name: obj_name,
                data: data[offset..offset + size].to_vec(),
            })
        })
        .collect()
}

/// Scanner for targeted asset extraction from IoStore containers.
///
/// Consolidates IoStore opening, schema loading, class resolution,
/// and parallel asset scanning into a single reusable type.
pub struct IoStoreScanner {
    store: Box<dyn IoStoreTrait>,
    toc_version: EIoStoreTocVersion,
    container_header_version: EIoContainerHeaderVersion,
    class_lookup: Option<Arc<HashMap<String, String>>>,
    name_to_hash: HashMap<String, String>,
    usmap_schema: Option<Arc<Usmap>>,
}

impl IoStoreScanner {
    /// Open an IoStore container (directory or single .utoc file).
    pub fn open(path: &Path, aes_key: Option<&str>) -> Result<Self> {
        let mut aes_keys = HashMap::new();
        if let Some(key) = aes_key {
            let parsed_key: AesKey = key
                .parse()
                .context("Invalid AES key format (use hex or base64)")?;
            aes_keys.insert(FGuid::default(), parsed_key);
        }
        let config = Arc::new(Config {
            aes_keys,
            container_header_version_override: None,
            toc_version_override: None,
        });

        let store =
            iostore::open(path, config).with_context(|| format!("Failed to open {:?}", path))?;

        let toc_version = store
            .container_file_version()
            .unwrap_or(EIoStoreTocVersion::ReplaceIoChunkHashWithIoHash);
        let container_header_version = store
            .container_header_version()
            .unwrap_or(EIoContainerHeaderVersion::NoExportInfo);

        Ok(Self {
            store,
            toc_version,
            container_header_version,
            class_lookup: None,
            name_to_hash: HashMap::new(),
            usmap_schema: None,
        })
    }

    /// Load scriptobjects.json for class name resolution.
    ///
    /// Must be called before `find_paths_by_class` or `scan_class`.
    pub fn load_scriptobjects(&mut self, path: &Path) -> Result<()> {
        let so_data = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read scriptobjects file {:?}", path))?;
        let so_json: serde_json::Value = serde_json::from_str(&so_data)
            .with_context(|| format!("Failed to parse scriptobjects file {:?}", path))?;

        let hash_to_path: HashMap<String, String> = so_json
            .get("hash_to_path")
            .and_then(|v| v.as_object())
            .context("scriptobjects.json missing hash_to_path")?
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
            .collect();

        // Build reverse lookup: class name → hash
        let mut name_to_hash = HashMap::new();
        if let Some(objects) = so_json.get("objects").and_then(|v| v.as_array()) {
            for obj in objects {
                let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let hash = obj.get("hash").and_then(|h| h.as_str()).unwrap_or("");
                if !name.is_empty() && !hash.is_empty() {
                    name_to_hash.insert(name.to_string(), hash.to_string());
                }
                // Also index by last segment of path (e.g., "GbxGame.InventoryBodyData" → "InventoryBodyData")
                if let Some(path) = obj.get("path").and_then(|p| p.as_str()) {
                    if let Some(last) = path.rsplit('.').next() {
                        if !name_to_hash.contains_key(last) {
                            name_to_hash.insert(last.to_string(), hash.to_string());
                        }
                    }
                }
            }
        }

        self.class_lookup = Some(Arc::new(hash_to_path));
        self.name_to_hash = name_to_hash;
        Ok(())
    }

    /// Load .usmap schema for property parsing.
    ///
    /// Without this, assets will be parsed without property resolution.
    pub fn load_usmap(&mut self, path: &Path) -> Result<()> {
        let usmap_data =
            std::fs::read(path).with_context(|| format!("Failed to read usmap {:?}", path))?;
        let usmap = Usmap::read(&mut Cursor::new(usmap_data))
            .with_context(|| format!("Failed to parse usmap {:?}", path))?;
        self.usmap_schema = Some(Arc::new(usmap));
        Ok(())
    }

    /// Find the class hash for a given class name.
    fn class_hash(&self, class_name: &str) -> Result<String> {
        self.name_to_hash
            .get(class_name)
            .cloned()
            .with_context(|| {
                format!(
                    "Class '{}' not found in scriptobjects (loaded {} classes)",
                    class_name,
                    self.name_to_hash.len()
                )
            })
    }

    /// Check if any export in a parsed header matches the target class hash.
    fn header_has_class(header: &FZenPackageHeader, target_hash: &str) -> bool {
        header.export_map.iter().any(|export| {
            export.class_index.kind() == FPackageObjectIndexType::ScriptImport
                && format!("{:X}", export.class_index.raw_index()) == target_hash
        })
    }

    /// Find all asset paths matching a class name.
    ///
    /// Requires `load_scriptobjects` to have been called first.
    pub fn find_paths_by_class(&self, class_name: &str) -> Result<Vec<String>> {
        let target_hash = self.class_hash(class_name)?;

        let uasset_entries: Vec<_> = self
            .store
            .chunks()
            .filter_map(|chunk| {
                let path = chunk.path()?;
                if path.ends_with(".uasset") {
                    Some((chunk, path))
                } else {
                    None
                }
            })
            .collect();

        let paths: Vec<String> = uasset_entries
            .par_iter()
            .filter_map(|(chunk, path)| {
                let data = chunk.read().ok()?;
                let mut cursor = Cursor::new(&data);
                let header = FZenPackageHeader::deserialize(
                    &mut cursor,
                    None,
                    self.toc_version,
                    self.container_header_version,
                    None,
                )
                .ok()?;
                if Self::header_has_class(&header, &target_hash) {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(paths)
    }

    /// Parse a single asset's raw data into structured form.
    pub fn parse_asset_data(&self, data: &[u8], path: &str) -> Result<ZenAssetInfo> {
        parse_zen_asset(
            data,
            path,
            self.toc_version,
            self.container_header_version,
            self.usmap_schema.as_ref(),
            self.class_lookup.as_ref(),
            false,
        )
    }

    /// Find and parse all assets of a given class.
    ///
    /// Combines `find_paths_by_class` with full property parsing in a single
    /// parallel pass over the IoStore.
    pub fn scan_class(&self, class_name: &str) -> Result<Vec<ZenAssetInfo>> {
        let target_hash = self.class_hash(class_name)?;

        let uasset_entries: Vec<_> = self
            .store
            .chunks()
            .filter_map(|chunk| {
                let path = chunk.path()?;
                if path.ends_with(".uasset") {
                    Some((chunk, path))
                } else {
                    None
                }
            })
            .collect();

        let results: Vec<ZenAssetInfo> = uasset_entries
            .par_iter()
            .filter_map(|(chunk, path)| {
                let data = chunk.read().ok()?;

                // Quick class check via header only
                let mut cursor = Cursor::new(&data);
                let header = FZenPackageHeader::deserialize(
                    &mut cursor,
                    None,
                    self.toc_version,
                    self.container_header_version,
                    None,
                )
                .ok()?;

                if !Self::header_has_class(&header, &target_hash) {
                    return None;
                }

                // Full parse with properties
                self.parse_asset_data(&data, path).ok()
            })
            .collect();

        Ok(results)
    }

    /// Find all assets of a given class and return raw export bytes.
    ///
    /// Like `scan_class`, but returns raw binary data per export instead of
    /// parsed properties. Use this for classes with native C++ serialization
    /// that need custom deserializers (e.g., GbxStatusEffectData).
    pub fn scan_class_raw(&self, class_name: &str) -> Result<Vec<RawExportData>> {
        let target_hash = self.class_hash(class_name)?;

        let uasset_entries: Vec<_> = self
            .store
            .chunks()
            .filter_map(|chunk| {
                let path = chunk.path()?;
                if path.ends_with(".uasset") {
                    Some((chunk, path))
                } else {
                    None
                }
            })
            .collect();

        let results: Vec<RawExportData> = uasset_entries
            .par_iter()
            .filter_map(|(chunk, path)| {
                let data = chunk.read().ok()?;
                let mut cursor = Cursor::new(&data);
                let header = FZenPackageHeader::deserialize(
                    &mut cursor,
                    None,
                    self.toc_version,
                    self.container_header_version,
                    None,
                )
                .ok()?;

                if !Self::header_has_class(&header, &target_hash) {
                    return None;
                }

                let header_end = cursor.position() as usize;
                let exports = extract_raw_exports(&header, &data, header_end, &target_hash);
                if exports.is_empty() {
                    return None;
                }

                Some(RawExportData {
                    path: path.clone(),
                    package_name: header
                        .name_map
                        .get(header.summary.name)
                        .to_string(),
                    name_map: header.name_map.copy_raw_names(),
                    exports,
                })
            })
            .collect();

        Ok(results)
    }

    /// Scan assets matching a path filter, parsing all matches.
    ///
    /// Useful for assets that share a common class but are distinguished by path
    /// (e.g., balance data under `balancedata/`).
    pub fn scan_by_path<F>(&self, filter: F) -> Result<Vec<ZenAssetInfo>>
    where
        F: Fn(&str) -> bool + Sync,
    {
        let uasset_entries: Vec<_> = self
            .store
            .chunks()
            .filter_map(|chunk| {
                let path = chunk.path()?;
                if path.ends_with(".uasset") && filter(&path) {
                    Some((chunk, path))
                } else {
                    None
                }
            })
            .collect();

        let results: Vec<ZenAssetInfo> = uasset_entries
            .par_iter()
            .filter_map(|(chunk, path)| {
                let data = chunk.read().ok()?;
                self.parse_asset_data(&data, path).ok()
            })
            .collect();

        Ok(results)
    }
}
