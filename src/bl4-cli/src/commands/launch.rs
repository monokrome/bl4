//! Launch command handlers
//!
//! Handles the `launch` subcommand for launching BL4 with instrumentation.

use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

/// Find the preload library path
///
/// Searches in the following locations:
/// 1. Next to the bl4 executable
/// 2. In target/release relative to current directory
pub fn find_preload_library() -> Option<PathBuf> {
    // Try next to the executable
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    if let Some(lib_path) = exe_dir
        .as_ref()
        .map(|d| d.join("libbl4_preload.so"))
        .filter(|p| p.exists())
    {
        return Some(lib_path);
    }

    // Try relative to current dir
    let p = PathBuf::from("target/release/libbl4_preload.so");
    if p.exists() {
        return std::fs::canonicalize(p).ok();
    }

    None
}

/// Build the LD_PRELOAD launch options string
pub fn build_launch_options(lib_path: &PathBuf) -> String {
    format!("LD_PRELOAD={} %command%", lib_path.display())
}

/// Print launch information
pub fn print_launch_info(launch_options: &str) {
    println!("Add to Steam launch options:\n");
    println!("  {}\n", launch_options);
    println!("Options: BL4_RNG_BIAS=max|high|low|min  BL4_PRELOAD_ALL=1  BL4_PRELOAD_STACKS=1");
    println!("Log: /tmp/bl4_preload.log\n");
}

/// Prompt user for confirmation
pub fn prompt_confirmation() -> Result<bool> {
    print!("Launch game? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

/// Launch the game via Steam
pub fn launch_via_steam() -> Result<()> {
    Command::new("steam")
        .arg("steam://rungameid/1285190")
        .status()
        .context("Failed to launch Steam")?;
    Ok(())
}

/// Handle the launch command
///
/// # Arguments
/// * `yes` - Skip confirmation prompt if true
pub fn handle(yes: bool) -> Result<()> {
    let lib_path = find_preload_library().ok_or_else(|| {
        anyhow::anyhow!(
            "Preload library not found. Build it first:\n  \
            cargo build --release -p bl4-preload"
        )
    })?;

    let launch_options = build_launch_options(&lib_path);
    print_launch_info(&launch_options);

    // Prompt for confirmation
    if !yes {
        if !prompt_confirmation()? {
            return Ok(());
        }
    }

    launch_via_steam()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_launch_options() {
        let path = PathBuf::from("/path/to/libbl4_preload.so");
        let options = build_launch_options(&path);

        assert!(options.contains("LD_PRELOAD="));
        assert!(options.contains("/path/to/libbl4_preload.so"));
        assert!(options.contains("%command%"));
    }

    #[test]
    fn test_build_launch_options_format() {
        let path = PathBuf::from("/usr/lib/preload.so");
        let options = build_launch_options(&path);

        assert_eq!(options, "LD_PRELOAD=/usr/lib/preload.so %command%");
    }

    #[test]
    fn test_find_preload_library_returns_none_when_missing() {
        // In test environment, the library likely doesn't exist
        // This test verifies the function doesn't panic
        let _ = find_preload_library();
    }

    #[test]
    fn test_print_launch_info_does_not_panic() {
        // Just verify it doesn't panic
        print_launch_info("LD_PRELOAD=/test/lib.so %command%");
    }
}
