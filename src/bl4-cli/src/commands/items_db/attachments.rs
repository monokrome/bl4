//! Attachment and import/export operations for items database

use anyhow::Result;
use bl4_idb::{AttachmentsRepository, ImportExportRepository, ItemsRepository};
use std::path::Path;

/// Handle `idb attach`
pub fn attach(
    db: &Path,
    image: &Path,
    serial: &str,
    name: Option<String>,
    popup: bool,
    detail: bool,
) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    let view = if popup {
        "POPUP"
    } else if detail {
        "DETAIL"
    } else {
        "OTHER"
    };
    let attachment_name = name.unwrap_or_else(|| {
        image
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let mime_type = match image.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };

    let data = std::fs::read(image)?;
    let attachment_id = wdb.add_attachment(serial, &attachment_name, mime_type, &data, view)?;
    println!(
        "Added attachment '{}' (ID {}, view: {}) to item {}",
        attachment_name, attachment_id, view, serial
    );
    Ok(())
}

/// Handle `idb import`
pub fn import(db: &Path, path: &Path) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    if path.join("serial.txt").exists() {
        let serial = wdb.import_from_dir(path)?;
        println!("Imported item {} from {}", serial, path.display());
    } else {
        let mut imported = 0;
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let subdir = entry.path();
            if subdir.is_dir() && subdir.join("serial.txt").exists() {
                match wdb.import_from_dir(&subdir) {
                    Ok(serial) => {
                        println!(
                            "Imported {} ({})",
                            subdir.file_name().unwrap_or_default().to_string_lossy(),
                            &serial[..serial.len().min(30)]
                        );
                        imported += 1;
                    }
                    Err(e) => eprintln!("Failed to import {}: {}", subdir.display(), e),
                }
            }
        }
        println!("\nImported {} items", imported);
    }
    Ok(())
}

/// Handle `idb export`
pub fn export(db: &Path, serial: &str, output: &Path) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.export_to_dir(serial, output)?;
    println!("Exported item {} to {}", serial, output.display());
    Ok(())
}
