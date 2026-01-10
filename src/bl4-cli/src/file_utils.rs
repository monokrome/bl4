//! File system utilities for common traversal patterns

use anyhow::Result;
use std::path::Path;

/// Walk files in a directory tree, filtering by extension
///
/// Calls the handler for each file matching the extension filter.
/// Extension should not include the dot (e.g., "bin" not ".bin").
pub fn walk_files_with_extension<F>(
    path: &Path,
    extensions: &[&str],
    mut handler: F,
) -> Result<()>
where
    F: FnMut(&Path) -> Result<()>,
{
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();

        let matches = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| extensions.iter().any(|ext| e.eq_ignore_ascii_case(ext)))
            .unwrap_or(false);

        if matches {
            handler(file_path)?;
        }
    }

    Ok(())
}

/// Collect files matching extension into a vector
pub fn collect_files_with_extension(path: &Path, extensions: &[&str]) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();

    walk_files_with_extension(path, extensions, |file_path| {
        files.push(file_path.to_path_buf());
        Ok(())
    })?;

    Ok(files)
}

/// Walk all subdirectories at depth 1 (immediate children only)
pub fn walk_subdirs<F>(path: &Path, mut handler: F) -> Result<()>
where
    F: FnMut(&Path) -> Result<()>,
{
    if !path.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let subpath = entry.path();
        if subpath.is_dir() {
            handler(&subpath)?;
        }
    }

    Ok(())
}
