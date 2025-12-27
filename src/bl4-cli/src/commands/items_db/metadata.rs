//! Metadata operations for items database (verification, sources, values)

use anyhow::Result;
use bl4_idb::ItemsRepository;
use std::path::Path;

/// Handle `idb verify`
pub fn verify(db: &Path, serial: &str, status: &str, notes: Option<String>) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;
    let status: bl4_idb::VerificationStatus = status.parse()?;
    wdb.set_verification_status(serial, status, notes.as_deref())?;
    println!("Updated item {} to status: {}", serial, status);
    Ok(())
}

/// Handle `idb mark-legal`
pub fn mark_legal(db: &Path, ids: &[String]) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    if ids.len() == 1 && ids[0] == "all" {
        let count = wdb.set_all_legal(true)?;
        println!("Marked all {} items as legal", count);
    } else {
        let mut marked = 0;
        for serial in ids {
            wdb.set_legal(serial, true)?;
            marked += 1;
        }
        println!("Marked {} items as legal", marked);
    }
    Ok(())
}

/// Handle `idb set-source`
pub fn set_source(
    db: &Path,
    source: &str,
    ids: &[String],
    where_clause: Option<String>,
) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    if let Some(condition) = where_clause {
        let count = wdb.set_source_where(source, &condition)?;
        println!("Set source to '{}' for {} items", source, count);
    } else if ids.len() == 1 && ids[0] == "null" {
        let count = wdb.set_source_for_null(source)?;
        println!(
            "Set source to '{}' for {} items with no source",
            source, count
        );
    } else {
        let mut updated = 0;
        for serial in ids {
            wdb.set_source(serial, source)?;
            updated += 1;
        }
        println!("Set source to '{}' for {} items", source, updated);
    }
    Ok(())
}

/// Handle `idb set-value`
pub fn set_value(
    db: &Path,
    serial: &str,
    field: &str,
    value: &str,
    source: &str,
    source_detail: Option<String>,
    confidence: &str,
) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    let source: bl4_idb::ValueSource = source.parse()?;
    let confidence: bl4_idb::Confidence = confidence.parse()?;

    wdb.set_value(
        serial,
        field,
        value,
        source,
        source_detail.as_deref(),
        confidence,
    )?;
    println!(
        "Set {}.{} = {} (source: {}, confidence: {})",
        serial, field, value, source, confidence
    );
    Ok(())
}

/// Handle `idb get-values`
pub fn get_values(db: &Path, serial: &str, field: &str) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    let values = wdb.get_values(serial, field)?;

    if values.is_empty() {
        println!("No values found for {}.{}", serial, field);
    } else {
        println!("Values for {}.{}:", serial, field);
        for v in values {
            println!(
                "  {} ({}, {}): {}",
                v.source,
                v.confidence,
                v.source_detail.as_deref().unwrap_or("-"),
                v.value
            );
        }
    }
    Ok(())
}

/// Handle `idb migrate-values`
pub fn migrate_values(db: &Path, dry_run: bool) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    if dry_run {
        println!("Dry run - showing what would be migrated:");
    }

    let stats = wdb.migrate_column_values(dry_run)?;

    println!();
    println!(
        "Migration {}:",
        if dry_run { "preview" } else { "complete" }
    );
    println!("  Items processed: {}", stats.items_processed);
    println!("  Values migrated: {}", stats.values_migrated);
    println!("  Values skipped (already exist): {}", stats.values_skipped);
    Ok(())
}
