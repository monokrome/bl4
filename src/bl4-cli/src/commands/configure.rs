//! Configuration command handlers
//!
//! Handles the `configure` subcommand for setting up bl4 CLI defaults.

use crate::config::Config;
use anyhow::Result;

/// Handle the configure command
///
/// # Arguments
/// * `steam_id` - Optional Steam ID to set as default
/// * `show` - If true, show current configuration
pub fn handle(steam_id: Option<String>, show: bool) -> Result<()> {
    let mut config = Config::load()?;

    if show {
        show_config(&config)?;
        return Ok(());
    }

    if let Some(id) = steam_id {
        set_steam_id(&mut config, id)?;
    } else {
        show_usage();
    }

    Ok(())
}

/// Display current configuration
fn show_config(config: &Config) -> Result<()> {
    if let Some(id) = config.get_steam_id() {
        println!("Steam ID: {}", id);
    } else {
        println!("No Steam ID configured");
    }

    if let Ok(path) = Config::config_path() {
        println!("Config file: {}", path.display());
    }

    Ok(())
}

/// Set the Steam ID in configuration
fn set_steam_id(config: &mut Config, id: String) -> Result<()> {
    config.set_steam_id(id.clone());
    config.save()?;

    println!("Steam ID configured: {}", id);
    if let Ok(path) = Config::config_path() {
        println!("Config saved to: {}", path.display());
    }

    Ok(())
}

/// Show usage help for the configure command
fn show_usage() {
    println!("Usage: bl4 configure --steam-id YOUR_STEAM_ID");
    println!("   or: bl4 configure --show");
    println!();
    println!("Note: Borderlands 4 uses your Steam ID to encrypt saves.");
    println!("      Find it in the top left of your Steam account page.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_usage_does_not_panic() {
        // Just verify it doesn't panic
        show_usage();
    }

    #[test]
    fn test_config_path_exists() {
        // Config::config_path() should return a valid path
        let result = Config::config_path();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_load() {
        // Should be able to load config (may be empty)
        let result = Config::load();
        assert!(result.is_ok());
    }
}
