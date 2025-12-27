//! Preload library command handlers
//!
//! Handles commands related to the LD_PRELOAD library for intercepting file I/O.

use anyhow::{bail, Context, Result};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Find the preload library path
///
/// Searches in the following locations:
/// 1. Next to the bl4 executable
/// 2. In target/release relative to current directory
pub fn find_preload_library() -> Option<PathBuf> {
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

    let p = PathBuf::from("target/release/libbl4_preload.so");
    if p.exists() {
        return std::fs::canonicalize(p).ok();
    }

    None
}

/// Handle preload info subcommand
pub fn handle_preload_info() -> Result<()> {
    match find_preload_library() {
        Some(p) => {
            println!("Preload library: {}", p.display());
            println!();
            println!("Usage:");
            println!("  LD_PRELOAD={} ./program", p.display());
            println!();
            println!("Environment variables:");
            println!("  BL4_PRELOAD_LOG=<path>     Log file (default: /tmp/bl4_preload.log)");
            println!("  BL4_PRELOAD_CAPTURE=<dir>  Save captured writes to directory");
            println!("  BL4_PRELOAD_FILTER=<pat>   Only capture files matching pattern");
            println!();
            println!("Note: For Wine/Proton apps, file paths are translated.");
            println!("      Hook catches Linux syscalls, not Windows API calls.");
            Ok(())
        }
        None => {
            bail!("Preload library not found. Build with: cargo build -p bl4-preload --release")
        }
    }
}

/// Handle preload run subcommand
pub fn handle_preload_run(
    capture: Option<&Path>,
    filter: Option<&str>,
    winedebug: Option<&str>,
    command: &[String],
) -> Result<()> {
    let lib = find_preload_library().context("Preload library not found")?;

    if command.is_empty() {
        bail!("No command specified");
    }

    let mut cmd = Command::new(&command[0]);
    cmd.args(&command[1..]);
    cmd.env("LD_PRELOAD", &lib);

    if let Some(dir) = capture {
        cmd.env("BL4_PRELOAD_CAPTURE", dir);
    }
    if let Some(f) = filter {
        cmd.env("BL4_PRELOAD_FILTER", f);
    }
    if let Some(debug) = winedebug {
        cmd.env("WINEDEBUG", debug);
    }

    println!("Running with LD_PRELOAD={}", lib.display());
    println!("Log: /tmp/bl4_preload.log");
    if let Some(dir) = capture {
        println!("Capture: {}", dir.display());
    }
    if let Some(debug) = winedebug {
        println!("WINEDEBUG: {}", debug);
    }
    println!();

    let status = cmd.status().context("Failed to run command")?;
    std::process::exit(status.code().unwrap_or(1));
}

/// Handle preload watch subcommand
pub fn handle_preload_watch(log_file: &Path) -> Result<()> {
    println!("Watching: {}", log_file.display());
    println!("Press Ctrl+C to stop\n");

    let file = std::fs::File::open(log_file)
        .with_context(|| format!("Failed to open {}", log_file.display()))?;
    let mut reader = std::io::BufReader::new(file);

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => std::thread::sleep(std::time::Duration::from_millis(100)),
            Ok(_) => print!("{}", line),
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_preload_library_handles_missing() {
        // This test just verifies the function doesn't panic when library isn't found
        let _ = find_preload_library();
    }
}
