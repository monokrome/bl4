//! Network operations for items database (publish, pull)

use anyhow::{bail, Result};
use bl4_idb::{AttachmentsRepository, ItemsRepository};
use std::collections::HashMap;
use std::path::Path;

/// Handle `idb publish`
pub fn publish(
    db: &Path,
    server: &str,
    serial: Option<String>,
    attachments: bool,
    dry_run: bool,
) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    let items = if let Some(serial) = serial {
        match wdb.get_item(&serial)? {
            Some(item) => vec![item],
            None => bail!("Item not found: {}", serial),
        }
    } else {
        wdb.list_items(&bl4_idb::ItemFilter::default())?
    };

    if items.is_empty() {
        println!("No items to publish");
        return Ok(());
    }

    // Check server capabilities if attachments requested
    let server_supports_attachments = if attachments {
        let caps_url = format!("{}/capabilities", server.trim_end_matches('/'));
        match ureq::get(&caps_url).call() {
            Ok(resp) => {
                let caps: serde_json::Value = resp.into_json()?;
                caps["attachments"].as_bool().unwrap_or(false)
            }
            Err(_) => {
                println!("Warning: Could not check server capabilities, skipping attachments");
                false
            }
        }
    } else {
        false
    };

    println!("Publishing {} items to {}", items.len(), server);
    if attachments && server_supports_attachments {
        println!("  Attachments: enabled");
    } else if attachments {
        println!("  Attachments: requested but server doesn't support them");
    }

    if dry_run {
        println!("\nDry run - would publish:");
        for item in &items {
            let attachment_count = wdb.get_attachments(&item.serial)?.len();
            if attachment_count > 0 && server_supports_attachments {
                println!("  {} ({} attachments)", item.serial, attachment_count);
            } else {
                println!("  {}", item.serial);
            }
        }
        return Ok(());
    }

    // Bulk fetch all item_values (1 query)
    let serials_for_values: Vec<&str> = items.iter().map(|i| i.serial.as_str()).collect();
    let all_values = wdb.get_all_values_bulk(&serials_for_values)?;

    // Group values by serial for quick lookup
    let mut values_by_serial: HashMap<&str, Vec<&bl4_idb::ItemValue>> = HashMap::new();
    for v in &all_values {
        values_by_serial.entry(&v.item_serial).or_default().push(v);
    }

    println!(
        "  Values: {} total across {} items",
        all_values.len(),
        values_by_serial.len()
    );

    // Group items by source for separate batch UUIDs on server
    let mut groups: HashMap<String, Vec<&bl4_idb::Item>> = HashMap::new();
    for item in &items {
        let source_key = item.source.clone().unwrap_or_default();
        groups.entry(source_key).or_default().push(item);
    }

    println!("  Groups: {} (by source)", groups.len());

    let url = format!("{}/items/bulk", server.trim_end_matches('/'));
    let mut total_succeeded = 0u64;
    let mut total_failed = 0u64;

    for (group_idx, (_source, group_items)) in groups.iter().enumerate() {
        let bulk_items: Vec<serde_json::Value> = group_items
            .iter()
            .map(|item| {
                let item_values: Vec<serde_json::Value> = values_by_serial
                    .get(item.serial.as_str())
                    .map(|vals| {
                        vals.iter()
                            .map(|v| {
                                serde_json::json!({
                                    "field": v.field,
                                    "value": v.value,
                                    "source": v.source.to_string(),
                                    "source_detail": v.source_detail,
                                    "confidence": v.confidence.to_string()
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                serde_json::json!({
                    "serial": item.serial,
                    "name": item.name,
                    "source": "bl4-cli",
                    "values": item_values
                })
            })
            .collect();

        let response = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({ "items": bulk_items }));

        match response {
            Ok(resp) => {
                let result: serde_json::Value = resp.into_json()?;
                let succeeded = result["succeeded"].as_u64().unwrap_or(0);
                let failed = result["failed"].as_u64().unwrap_or(0);
                total_succeeded += succeeded;
                total_failed += failed;

                if let Some(batch_id) = result["batch_id"].as_str() {
                    println!(
                        "  Group {}: {} items -> batch {}",
                        group_idx + 1,
                        group_items.len(),
                        batch_id
                    );
                }

                if let Some(results) = result["results"].as_array() {
                    for r in results {
                        if !r["created"].as_bool().unwrap_or(true) {
                            println!("    {} - {}", r["serial"], r["message"]);
                        }
                    }
                }
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                println!("  Group {} failed: {} - {}", group_idx + 1, code, body);
                total_failed += group_items.len() as u64;
            }
            Err(e) => {
                println!("  Group {} failed: {}", group_idx + 1, e);
                total_failed += group_items.len() as u64;
            }
        }
    }

    println!("\nPublish complete:");
    println!("  Items succeeded: {}", total_succeeded);
    println!("  Items failed: {}", total_failed);

    // Upload attachments if enabled
    if server_supports_attachments {
        upload_attachments(&wdb, &items, server)?;
    }
    Ok(())
}

fn upload_attachments(
    wdb: &bl4_idb::SqliteDb,
    items: &[bl4_idb::Item],
    server: &str,
) -> Result<()> {
    let mut attachments_uploaded = 0;
    let mut attachments_failed = 0;

    let serials: Vec<&str> = items.iter().map(|i| i.serial.as_str()).collect();
    let all_attachments = wdb.get_attachments_bulk(&serials)?;

    if all_attachments.is_empty() {
        return Ok(());
    }

    let attachment_ids: Vec<i64> = all_attachments.iter().map(|a| a.id).collect();
    let attachment_data: HashMap<i64, Vec<u8>> = wdb
        .get_attachment_data_bulk(&attachment_ids)?
        .into_iter()
        .collect();

    for attachment in all_attachments {
        let data = match attachment_data.get(&attachment.id) {
            Some(d) => d,
            None => continue,
        };

        let upload_url = format!(
            "{}/items/{}/attachments",
            server.trim_end_matches('/'),
            urlencoding::encode(&attachment.item_serial)
        );

        let boundary = "----bl4clipublish";
        let mut body = Vec::new();

        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                attachment.name
            )
            .as_bytes(),
        );
        body.extend_from_slice(
            format!("Content-Type: {}\r\n\r\n", attachment.mime_type).as_bytes(),
        );
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");

        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"view\"\r\n\r\n");
        body.extend_from_slice(attachment.view.as_bytes());
        body.extend_from_slice(b"\r\n");

        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let result = ureq::post(&upload_url)
            .set(
                "Content-Type",
                &format!("multipart/form-data; boundary={}", boundary),
            )
            .send_bytes(&body);

        match result {
            Ok(_) => attachments_uploaded += 1,
            Err(e) => {
                eprintln!(
                    "Failed to upload attachment {} for {}: {}",
                    attachment.name, attachment.item_serial, e
                );
                attachments_failed += 1;
            }
        }
    }

    if attachments_uploaded > 0 || attachments_failed > 0 {
        println!("\nAttachments:");
        println!("  Uploaded: {}", attachments_uploaded);
        if attachments_failed > 0 {
            println!("  Failed: {}", attachments_failed);
        }
    }
    Ok(())
}

/// Handle `idb pull`
pub fn pull(db: &Path, server: &str, authoritative: bool, dry_run: bool) -> Result<()> {
    use bl4_idb::{Confidence, ValueSource};

    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    println!("Fetching items from {}...", server);
    if authoritative {
        println!("  Mode: authoritative (remote values will overwrite local)");
    }

    // Fetch all items from server (paginated)
    let all_items = fetch_all_items(server)?;

    if all_items.is_empty() {
        println!("No items to pull");
        return Ok(());
    }

    println!("\nPulled {} items from server", all_items.len());

    if dry_run {
        return handle_dry_run(&wdb, &all_items, authoritative);
    }

    // Merge into local database
    let mut new_items = 0;
    let mut updated_items = 0;
    let mut values_set = 0;

    let field_mappings = [
        ("name", "name"),
        ("prefix", "prefix"),
        ("manufacturer", "manufacturer"),
        ("weapon_type", "weapon_type"),
        ("rarity", "rarity"),
        ("level", "level"),
        ("element", "element"),
        ("item_type", "item_type"),
    ];

    for item in &all_items {
        let serial = match item["serial"].as_str() {
            Some(s) => s,
            None => continue,
        };

        let is_new = wdb.get_item(serial)?.is_none();

        if is_new {
            if let Err(e) = wdb.add_item(serial) {
                eprintln!("Failed to add {}: {}", serial, e);
                continue;
            }
            new_items += 1;
        } else if !authoritative {
            continue;
        } else {
            updated_items += 1;
        }

        for (json_key, field_name) in &field_mappings {
            let value = if *json_key == "level" {
                item[*json_key].as_i64().map(|v| v.to_string())
            } else {
                item[*json_key].as_str().map(String::from)
            };

            if let Some(val) = value {
                if !val.is_empty() {
                    let _ = wdb.set_value(
                        serial,
                        field_name,
                        &val,
                        ValueSource::CommunityTool,
                        Some(server),
                        Confidence::Uncertain,
                    );
                    values_set += 1;
                }
            }
        }

        let _ = wdb.set_source(serial, "community-pull");
    }

    println!("\nPull complete:");
    println!("  New items: {}", new_items);
    if authoritative {
        println!("  Updated items: {}", updated_items);
    }
    println!("  Values set: {}", values_set);
    Ok(())
}

fn fetch_all_items(server: &str) -> Result<Vec<serde_json::Value>> {
    let mut all_items: Vec<serde_json::Value> = Vec::new();
    let mut offset = 0;
    let limit = 1000;

    loop {
        let url = format!(
            "{}/items?limit={}&offset={}",
            server.trim_end_matches('/'),
            limit,
            offset
        );

        let response = ureq::get(&url).call();

        match response {
            Ok(resp) => {
                let result: serde_json::Value = resp.into_json()?;
                let items = result["items"].as_array();
                let total = result["total"].as_u64().unwrap_or(0);

                if let Some(items) = items {
                    if items.is_empty() {
                        break;
                    }
                    all_items.extend(items.clone());
                    println!("  Fetched {} / {} items", all_items.len(), total);

                    if all_items.len() >= total as usize {
                        break;
                    }
                    offset += limit;
                } else {
                    break;
                }
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                bail!("Server returned {}: {}", code, body);
            }
            Err(e) => {
                bail!("Request failed: {}", e);
            }
        }
    }
    Ok(all_items)
}

fn handle_dry_run(
    wdb: &bl4_idb::SqliteDb,
    all_items: &[serde_json::Value],
    authoritative: bool,
) -> Result<()> {
    println!("\nDry run - would process:");
    let mut new_items = 0;
    let mut existing_items = 0;
    for item in all_items {
        if let Some(serial) = item["serial"].as_str() {
            if wdb.get_item(serial)?.is_none() {
                println!("  [NEW] {}", serial);
                new_items += 1;
            } else {
                existing_items += 1;
            }
        }
    }
    println!("\n{} new items, {} existing", new_items, existing_items);
    if authoritative {
        println!(
            "With --authoritative, values for all {} items would be updated",
            all_items.len()
        );
    } else {
        println!("Without --authoritative, only new items would get values");
    }
    Ok(())
}
