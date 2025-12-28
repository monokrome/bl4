//! Decoding and save import operations for items database

use anyhow::{bail, Context, Result};
use bl4_idb::ItemsRepository;
use std::path::Path;

use super::helpers::extract_serials_from_yaml;

/// Handle `idb decode-all`
pub fn decode_all(db: &Path, force: bool) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;
    let items = wdb.list_items(&bl4_idb::ItemFilter::default())?;

    let mut decoded = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for item in &items {
        if !force && (item.manufacturer.is_some() || item.weapon_type.is_some()) {
            skipped += 1;
            continue;
        }

        match bl4::ItemSerial::decode(&item.serial) {
            Ok(decoded_item) => {
                let (mfg, wtype) = if let Some(mfg_id) = decoded_item.manufacturer {
                    bl4::parts::weapon_info_from_first_varint(mfg_id)
                        .map(|(m, w)| (Some(m.to_string()), Some(w.to_string())))
                        .unwrap_or((None, None))
                } else if let Some(group_id) = decoded_item.part_group_id() {
                    let cat_name =
                        bl4::parts::category_name_for_type(decoded_item.item_type, group_id);
                    (None, cat_name.map(|s| s.to_string()))
                } else {
                    (None, None)
                };

                let level = decoded_item
                    .level
                    .and_then(bl4::parts::level_from_code)
                    .map(|(capped, _raw)| capped as i32);

                let update = bl4_idb::ItemUpdate {
                    manufacturer: mfg,
                    weapon_type: wtype,
                    level,
                    ..Default::default()
                };
                wdb.update_item(&item.serial, &update)?;
                wdb.set_item_type(&item.serial, &decoded_item.item_type.to_string())?;

                if item.verification_status == bl4_idb::VerificationStatus::Unverified {
                    wdb.set_verification_status(
                        &item.serial,
                        bl4_idb::VerificationStatus::Decoded,
                        None,
                    )?;
                }
                decoded += 1;
            }
            Err(e) => {
                eprintln!("Failed to decode {}: {}", item.serial, e);
                failed += 1;
            }
        }
    }
    println!(
        "Decoded {} items, skipped {} (already decoded), {} failed",
        decoded, skipped, failed
    );
    Ok(())
}

/// Handle `idb decode`
pub fn decode(db: &Path, serial: Option<String>, all: bool) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    let serials: Vec<String> = if let Some(s) = serial {
        vec![s]
    } else if all {
        wdb.list_items(&bl4_idb::ItemFilter::default())?
            .into_iter()
            .map(|i| i.serial)
            .collect()
    } else {
        bail!("Either provide a serial or use --all");
    };

    let mut decoded_count = 0;
    let mut values_set = 0;
    let mut failed = 0;

    for serial in &serials {
        match bl4::ItemSerial::decode(serial) {
            Ok(item) => {
                // Extract level
                if let Some(level_code) = item.level {
                    if let Some((capped, _raw)) = bl4::parts::level_from_code(level_code) {
                        wdb.set_value(
                            serial,
                            "level",
                            &capped.to_string(),
                            bl4_idb::ValueSource::Decoder,
                            Some("bl4-cli"),
                            bl4_idb::Confidence::Inferred,
                        )?;
                        values_set += 1;
                    }
                }

                // Extract manufacturer and weapon_type from first varint
                if let Some(mfg_id) = item.manufacturer {
                    if let Some((mfg, wtype)) = bl4::parts::weapon_info_from_first_varint(mfg_id) {
                        wdb.set_value(
                            serial,
                            "manufacturer",
                            mfg,
                            bl4_idb::ValueSource::Decoder,
                            Some("bl4-cli"),
                            bl4_idb::Confidence::Inferred,
                        )?;
                        values_set += 1;

                        wdb.set_value(
                            serial,
                            "weapon_type",
                            wtype,
                            bl4_idb::ValueSource::Decoder,
                            Some("bl4-cli"),
                            bl4_idb::Confidence::Inferred,
                        )?;
                        values_set += 1;
                    }
                } else if let Some(group_id) = item.part_group_id() {
                    if let Some(cat_name) =
                        bl4::parts::category_name_for_type(item.item_type, group_id)
                    {
                        wdb.set_value(
                            serial,
                            "weapon_type",
                            cat_name,
                            bl4_idb::ValueSource::Decoder,
                            Some("bl4-cli"),
                            bl4_idb::Confidence::Inferred,
                        )?;
                        values_set += 1;
                    }
                }

                // Set item_type
                wdb.set_value(
                    serial,
                    "item_type",
                    &item.item_type.to_string(),
                    bl4_idb::ValueSource::Decoder,
                    Some("bl4-cli"),
                    bl4_idb::Confidence::Inferred,
                )?;
                values_set += 1;

                decoded_count += 1;
            }
            Err(e) => {
                eprintln!("Failed to decode {}: {}", serial, e);
                failed += 1;
            }
        }
    }

    println!(
        "Decoded {} items, set {} values, {} failed",
        decoded_count, values_set, failed
    );
    Ok(())
}

/// Handle `idb import-save`
pub fn import_save(
    db: &Path,
    save: &Path,
    do_decode: bool,
    legal: bool,
    source: Option<String>,
) -> Result<()> {
    // Try to extract Steam ID from path first
    let steam_id = save
        .to_string_lossy()
        .split('/')
        .find(|s| s.len() == 17 && s.chars().all(|c| c.is_ascii_digit()))
        .map(String::from)
        .or_else(|| {
            save.parent()
                .map(|dir| dir.join("steamid"))
                .filter(|p| p.exists())
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| s.trim().to_string())
        })
        .context("Could not extract Steam ID from path or steamid file")?;

    let save_data = std::fs::read(save)?;
    let yaml_data = bl4::decrypt_sav(&save_data, &steam_id)?;
    let yaml_str = String::from_utf8(yaml_data)?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_str)?;

    let mut serials = Vec::new();
    extract_serials_from_yaml(&yaml, &mut serials);
    serials.sort();
    serials.dedup();

    println!(
        "Found {} unique serials in {}",
        serials.len(),
        save.display()
    );

    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;
    let mut added = 0;
    let mut skipped = 0;

    for serial in &serials {
        match wdb.add_item(serial) {
            Ok(_) => added += 1,
            Err(_) => skipped += 1,
        }
    }
    println!("Added {} new items, {} already existed", added, skipped);

    if do_decode && added > 0 {
        println!("Decoding new items...");
        let items = wdb.list_items(&bl4_idb::ItemFilter::default())?;
        let mut decoded_count = 0;

        for item in &items {
            if item.manufacturer.is_some() {
                continue;
            }
            if let Ok(decoded_item) = bl4::ItemSerial::decode(&item.serial) {
                let (mfg, wtype) = if let Some(mfg_id) = decoded_item.manufacturer {
                    bl4::parts::weapon_info_from_first_varint(mfg_id)
                        .map(|(m, w)| (Some(m.to_string()), Some(w.to_string())))
                        .unwrap_or((None, None))
                } else {
                    (None, None)
                };

                let level = decoded_item
                    .level
                    .and_then(bl4::parts::level_from_code)
                    .map(|(capped, _)| capped as i32);

                let update = bl4_idb::ItemUpdate {
                    manufacturer: mfg,
                    weapon_type: wtype,
                    level,
                    ..Default::default()
                };
                let _ = wdb.update_item(&item.serial, &update);

                if item.verification_status == bl4_idb::VerificationStatus::Unverified {
                    let _ = wdb.set_verification_status(
                        &item.serial,
                        bl4_idb::VerificationStatus::Decoded,
                        None,
                    );
                }
                decoded_count += 1;
            }
        }
        println!("Decoded {} items", decoded_count);
    }

    if legal {
        let mut marked = 0;
        for serial in &serials {
            if let Ok(Some(item)) = wdb.get_item(serial) {
                if !item.legal {
                    let _ = wdb.set_legal(&item.serial, true);
                    marked += 1;
                }
            }
        }
        println!("Marked {} items as legal", marked);
    }

    if let Some(src) = source {
        let mut updated = 0;
        for serial in &serials {
            if wdb.set_source(serial, &src).is_ok() {
                updated += 1;
            }
        }
        println!("Set source '{}' for {} items", src, updated);
    }
    Ok(())
}
