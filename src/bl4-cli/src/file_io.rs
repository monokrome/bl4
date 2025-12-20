//! I/O helpers for consistent file/stdin/stdout handling

use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

/// Read bytes from a file path or stdin if path is None
pub fn read_input(path: Option<&Path>) -> Result<Vec<u8>> {
    match path {
        Some(p) => fs::read(p).with_context(|| format!("Failed to read {}", p.display())),
        None => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .context("Failed to read from stdin")?;
            Ok(buf)
        }
    }
}

/// Write bytes to a file path or stdout if path is None
pub fn write_output(path: Option<&Path>, data: &[u8]) -> Result<()> {
    match path {
        Some(p) => fs::write(p, data).with_context(|| format!("Failed to write {}", p.display())),
        None => io::stdout()
            .write_all(data)
            .context("Failed to write to stdout"),
    }
}
