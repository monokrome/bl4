//! Object listing command handlers
//!
//! Handlers for listing UClass instances and UObjects from memory.

use crate::memory::{self, MemorySource};
use anyhow::{Context, Result};
use byteorder::ByteOrder;
use std::collections::HashMap;

/// Handle the ListUClasses command
///
/// Lists all UClass instances in memory.
pub fn handle_list_uclasses(
    source: &dyn MemorySource,
    limit: usize,
    filter: Option<&str>,
) -> Result<()> {
    // Discover FNamePool to resolve names
    let _gnames = memory::discover_gnames(source).context("Failed to find GNames pool")?;
    let pool = memory::FNamePool::discover(source).context("Failed to discover FNamePool")?;
    let mut fname_reader = memory::FNameReader::new(pool);

    // Find all UClass instances
    println!(
        "Finding all UClass instances (ClassPrivate == {:#x})...\n",
        memory::UCLASS_METACLASS_ADDR
    );

    let classes = memory::find_all_uclasses(source, &mut fname_reader)
        .context("Failed to enumerate UClass instances")?;

    // Apply filter if provided
    let filtered: Vec<_> = if let Some(pattern) = filter {
        let pattern_lower = pattern.to_lowercase();
        classes
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&pattern_lower))
            .collect()
    } else {
        classes.iter().collect()
    };

    println!(
        "Found {} UClass instances{}\n",
        filtered.len(),
        filter
            .map(|f| format!(" matching '{}'", f))
            .unwrap_or_default()
    );

    // Show results
    let show_count = if limit == 0 {
        filtered.len()
    } else {
        limit.min(filtered.len())
    };
    for class in filtered.iter().take(show_count) {
        println!(
            "  {:#x}: {} (FName {})",
            class.address, class.name, class.name_index
        );
    }

    if show_count < filtered.len() {
        println!(
            "\n  ... and {} more (use --limit 0 to show all)",
            filtered.len() - show_count
        );
    }

    // Show some stats
    let game_classes: Vec<_> = filtered
        .iter()
        .filter(|c| c.name.starts_with("U") || c.name.starts_with("A") || c.name.contains("_"))
        .collect();
    let core_classes: Vec<_> = filtered
        .iter()
        .filter(|c| !c.name.starts_with("U") && !c.name.starts_with("A") && !c.name.contains("_"))
        .collect();

    println!("\nClass categories:");
    println!("  Game classes (U*/A*/*_*): {}", game_classes.len());
    println!("  Core/Native classes: {}", core_classes.len());

    Ok(())
}

/// Handle the ListObjects command
///
/// Enumerates UObjects from GUObjectArray with optional filtering.
pub fn handle_list_objects(
    source: &dyn MemorySource,
    limit: usize,
    class_filter: Option<&str>,
    name_filter: Option<&str>,
    stats: bool,
) -> Result<()> {
    // Discover GNames first (needed for FName resolution)
    eprintln!("Searching for GNames pool...");
    let gnames = memory::discover_gnames(source).context("Failed to discover GNames")?;
    eprintln!("GNames found at: {:#x}\n", gnames.address);

    // Discover GUObjectArray via pattern-based search
    eprintln!("Searching for GUObjectArray...");
    let guobj = memory::discover_guobject_array(source, gnames.address)
        .context("Failed to discover GUObjectArray")?;

    // Discover FNamePool for name reading
    let pool = memory::FNamePool::discover(source).context("Failed to discover FNamePool")?;
    let mut fname_reader = memory::FNameReader::new(pool);

    println!("Enumerating UObjects from GUObjectArray...");
    println!("  Total objects: {}", guobj.num_elements);
    println!("  Item size: {} bytes\n", guobj.item_size);

    // Statistics tracking
    let mut total_valid = 0usize;
    let mut class_counts: HashMap<String, usize> = HashMap::new();
    let mut shown = 0usize;

    let class_filter_lower = class_filter.map(|s| s.to_lowercase());
    let name_filter_lower = name_filter.map(|s| s.to_lowercase());

    // Iterate over all objects
    for (idx, obj_ptr) in guobj.iter_objects(source) {
        // Read UObject header
        let obj_data = match source.read_bytes(obj_ptr, memory::UOBJECT_HEADER_SIZE) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let class_ptr = byteorder::LE::read_u64(
            &obj_data[memory::UOBJECT_CLASS_OFFSET..memory::UOBJECT_CLASS_OFFSET + 8],
        ) as usize;
        let name_idx = byteorder::LE::read_u32(
            &obj_data[memory::UOBJECT_NAME_OFFSET..memory::UOBJECT_NAME_OFFSET + 4],
        );

        // Read object name
        let obj_name = fname_reader
            .read_name(source, name_idx)
            .unwrap_or_else(|_| format!("FName_{}", name_idx));

        // Read class name (need to read the class object's name)
        let class_name = if class_ptr != 0 {
            if let Ok(class_data) = source.read_bytes(class_ptr, memory::UOBJECT_HEADER_SIZE) {
                let class_name_idx = byteorder::LE::read_u32(
                    &class_data[memory::UOBJECT_NAME_OFFSET..memory::UOBJECT_NAME_OFFSET + 4],
                );
                fname_reader
                    .read_name(source, class_name_idx)
                    .unwrap_or_else(|_| format!("FName_{}", class_name_idx))
            } else {
                "Unknown".to_string()
            }
        } else {
            "Null".to_string()
        };

        total_valid += 1;
        *class_counts.entry(class_name.clone()).or_insert(0) += 1;

        // Apply filters
        let class_match = class_filter_lower
            .as_ref()
            .map(|f| class_name.to_lowercase().contains(f))
            .unwrap_or(true);
        let name_match = name_filter_lower
            .as_ref()
            .map(|f| obj_name.to_lowercase().contains(f))
            .unwrap_or(true);

        if class_match && name_match && !stats && shown < limit {
            println!("[{}] {:#x}: {} ({})", idx, obj_ptr, obj_name, class_name);
            shown += 1;
        }

        // Progress indicator
        if total_valid.is_multiple_of(50000) {
            eprint!("\r  Scanned {} objects...", total_valid);
        }
    }

    eprintln!("\r  Scanned {} valid objects total.", total_valid);

    if stats || class_filter.is_some() || name_filter.is_some() {
        println!("\nStatistics:");
        println!("  Total valid objects: {}", total_valid);
        println!("  Unique classes: {}", class_counts.len());

        // Sort classes by count and show top 20
        let mut sorted_classes: Vec<_> = class_counts.into_iter().collect();
        sorted_classes.sort_by(|a, b| b.1.cmp(&a.1));

        println!("\nTop 20 classes by instance count:");
        for (class_name, count) in sorted_classes.iter().take(20) {
            println!("  {:6} {}", count, class_name);
        }
    }

    if !stats && shown >= limit && limit > 0 {
        println!(
            "\n... showing first {} matches (use --limit N to see more)",
            limit
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    #[test]
    fn test_handle_list_uclasses_empty_source() {
        let source = MockMemorySource::new(vec![], 0x1000);
        // Will fail to find GNames, which is expected
        let result = handle_list_uclasses(&source, 10, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_list_objects_empty_source() {
        let source = MockMemorySource::new(vec![], 0x1000);
        // Will fail to find GNames, which is expected
        let result = handle_list_objects(&source, 10, None, None, false);
        assert!(result.is_err());
    }
}
