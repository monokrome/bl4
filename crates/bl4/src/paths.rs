//! Default save file path detection for Borderlands 4
//!
//! This module provides utilities to detect the standard save file locations
//! for Borderlands 4 on different platforms.

use std::path::{Path, PathBuf};

/// Detect the default Borderlands 4 save directory for the current platform.
///
/// This checks standard locations where Borderlands 4 stores save files:
/// - **Windows**: `%USERPROFILE%\Documents\My Games\Borderlands 4\Saved\SaveGames\<steamid>\Profiles\client`
/// - **Linux (Proton)**: `~/.local/share/Steam/steamapps/compatdata/1285190/pfx/drive_c/users/steamuser/Documents/My Games/Borderlands 4/Saved/SaveGames/<steamid>/Profiles/client`
///
/// Returns the first valid save directory found, or `None` if no standard location exists.
///
/// # Example
///
/// ```no_run
/// if let Some(save_dir) = bl4::detect_save_directory() {
///     println!("Found saves at: {}", save_dir.display());
/// }
/// ```
pub fn detect_save_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        detect_windows_save_directory()
    }

    #[cfg(target_os = "linux")]
    {
        detect_linux_save_directory()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// Extract a Steam ID from a save directory path.
///
/// Looks for a 17-digit Steam ID (starting with 7656) in the directory path hierarchy.
/// This is useful for automatically detecting the Steam ID when loading save files.
///
/// # Example
///
/// ```
/// use std::path::PathBuf;
/// use bl4::extract_steam_id_from_path;
///
/// let path = PathBuf::from("/home/user/.local/share/Steam/steamapps/compatdata/1285190/pfx/drive_c/users/steamuser/Documents/My Games/Borderlands 4/Saved/SaveGames/76561197960521364/Profiles/client");
/// assert_eq!(extract_steam_id_from_path(&path), Some("76561197960521364".to_string()));
/// ```
pub fn extract_steam_id_from_path(path: &Path) -> Option<String> {
    let mut current = path;
    while let Some(parent) = current.parent() {
        if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
            // Steam IDs are typically 17 digits starting with 7656
            if name.len() == 17
                && name.starts_with("7656")
                && name.chars().all(|c| c.is_ascii_digit())
            {
                return Some(name.to_string());
            }
        }
        current = parent;
    }
    None
}

#[cfg(target_os = "windows")]
fn detect_windows_save_directory() -> Option<PathBuf> {
    // Windows: %USERPROFILE%\Documents\My Games\Borderlands 4\Saved\SaveGames\<steamid>\Profiles\client
    let userprofile = std::env::var("USERPROFILE").ok()?;
    let base = PathBuf::from(userprofile)
        .join("Documents")
        .join("My Games")
        .join("Borderlands 4")
        .join("Saved")
        .join("SaveGames");

    if !base.exists() {
        return None;
    }

    find_profiles_in_base(&base)
}

#[cfg(target_os = "linux")]
fn detect_linux_save_directory() -> Option<PathBuf> {
    // Linux (Proton): Steam saves are in compatdata with the app ID 1285190
    // Path structure: steamapps/compatdata/1285190/pfx/drive_c/users/steamuser/Documents/My Games/Borderlands 4/Saved/SaveGames/<steamid>/Profiles/client

    let bl4_app_id = "1285190";
    let proton_suffix = format!(
        "steamapps/compatdata/{}/pfx/drive_c/users/steamuser/Documents/My Games/Borderlands 4/Saved/SaveGames",
        bl4_app_id
    );

    // Try XDG_DATA_HOME first
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        let base = PathBuf::from(xdg_data).join("Steam").join(&proton_suffix);
        if let Some(profiles) = find_profiles_in_base(&base) {
            return Some(profiles);
        }
    }

    // Try ~/.local/share/Steam (default XDG_DATA_HOME)
    if let Ok(home) = std::env::var("HOME") {
        let base = PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("Steam")
            .join(&proton_suffix);
        if let Some(profiles) = find_profiles_in_base(&base) {
            return Some(profiles);
        }

        // Try ~/.steam/steam/steamapps/...
        let base = PathBuf::from(&home)
            .join(".steam")
            .join("steam")
            .join(&proton_suffix);
        if let Some(profiles) = find_profiles_in_base(&base) {
            return Some(profiles);
        }

        // Try ~/.steam/steamapps/... (simplified path)
        let base = PathBuf::from(&home).join(".steam").join(&proton_suffix);
        if let Some(profiles) = find_profiles_in_base(&base) {
            return Some(profiles);
        }
    }

    None
}

fn find_profiles_in_base(base: &PathBuf) -> Option<PathBuf> {
    if !base.exists() {
        return None;
    }

    // Find the first steamid directory and look for Profiles/client
    for entry in std::fs::read_dir(base).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() {
            // Check for both Profiles and Profiles/client
            let profiles_path = path.join("Profiles");
            let client_path = profiles_path.join("client");

            // Prefer Profiles/client if it exists, otherwise use Profiles
            if client_path.exists() {
                return Some(client_path);
            } else if profiles_path.exists() {
                return Some(profiles_path);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_steam_id_from_path() {
        let path = PathBuf::from("/home/user/SaveGames/76561197960521364/Profiles/client");
        assert_eq!(
            extract_steam_id_from_path(&path),
            Some("76561197960521364".to_string())
        );

        // Test with no Steam ID
        let path = PathBuf::from("/home/user/SaveGames/notanid/Profiles");
        assert_eq!(extract_steam_id_from_path(&path), None);

        // Test with wrong length
        let path = PathBuf::from("/home/user/SaveGames/123456/Profiles");
        assert_eq!(extract_steam_id_from_path(&path), None);
    }
}
