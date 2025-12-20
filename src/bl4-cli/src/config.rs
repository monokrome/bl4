//! Configuration management for bl4 CLI

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub steam_id: Option<String>,
}

impl Config {
    /// Get the path to the config file
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("bl4");

        Ok(config_dir.join("config.toml"))
    }

    /// Load configuration from file, or create default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Config::default());
        }

        let contents = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;

        toml::from_str(&contents).context("Failed to parse config file")
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory at {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

        Ok(())
    }

    /// Get Steam ID from config or None if not set
    pub fn get_steam_id(&self) -> Option<&str> {
        self.steam_id.as_deref()
    }

    /// Set Steam ID in config
    pub fn set_steam_id(&mut self, steam_id: String) {
        self.steam_id = Some(steam_id);
    }
}
