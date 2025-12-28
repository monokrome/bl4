//! Object discovery command handlers
//!
//! Handlers for finding and mapping UObjects from memory dumps.

use crate::memory::{self, MemorySource};
use anyhow::Result;
use std::path::Path;

/// Handle the FindObjectsByPattern command
///
/// Searches for objects matching a name pattern in a memory dump.
/// Requires a dump file - does not work with live process.
pub fn handle_find_objects_by_pattern(
    pattern: &str,
    limit: usize,
    dump: Option<&Path>,
) -> Result<()> {
    let dump_path = dump.ok_or_else(|| {
        anyhow::anyhow!("FindObjectsByPattern requires a memory dump file. Use --dump <path>")
    })?;

    println!("Searching for objects matching '{}'...", pattern);
    let source: Box<dyn MemorySource> = Box::new(memory::DumpFile::open(dump_path)?);

    // Discover GUObjectArray
    println!("Discovering GNames pool...");
    let gnames = memory::discover_gnames(source.as_ref())?;
    println!("  GNames at: {:#x}", gnames.address);

    println!("Discovering GUObjectArray...");
    let guobjects = memory::discover_guobject_array(source.as_ref(), gnames.address)?;
    println!("  GUObjectArray at: {:#x}", guobjects.address);
    println!("  NumElements: {}", guobjects.num_elements);

    // Find objects
    let results = memory::find_objects_by_pattern(source.as_ref(), &guobjects, pattern, limit)?;

    println!("\nResults:");
    for (name, class_name, class_ptr) in &results {
        println!("  '{}' (class: {} @ {:#x})", name, class_name, class_ptr);
    }

    Ok(())
}

/// Handle the GenerateObjectMap command
///
/// Generates a JSON map of all UObjects grouped by class.
/// Requires a dump file - does not work with live process.
pub fn handle_generate_object_map(output: Option<&Path>, dump: Option<&Path>) -> Result<()> {
    let dump_path = dump.ok_or_else(|| {
        anyhow::anyhow!("GenerateObjectMap requires a memory dump file. Use --dump <path>")
    })?;

    // Default output path is next to the dump file
    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let mut p = dump_path.to_path_buf();
            p.set_extension("objects.json");
            p
        }
    };

    println!("Generating object map from {}...", dump_path.display());
    let source: Box<dyn MemorySource> = Box::new(memory::DumpFile::open(dump_path)?);

    // Discover GUObjectArray
    println!("Discovering GNames pool...");
    let gnames = memory::discover_gnames(source.as_ref())?;
    println!("  GNames at: {:#x}", gnames.address);

    println!("Discovering GUObjectArray...");
    let guobjects = memory::discover_guobject_array(source.as_ref(), gnames.address)?;
    println!("  GUObjectArray at: {:#x}", guobjects.address);
    println!("  NumElements: {}", guobjects.num_elements);

    // Generate object map
    let map = memory::generate_object_map(source.as_ref(), &guobjects)?;

    // Write JSON output
    write_object_map_json(&output_path, &map)?;

    println!("Object map written to: {}", output_path.display());
    println!(
        "  {} classes, {} total objects",
        map.len(),
        map.values().map(|v| v.len()).sum::<usize>()
    );

    Ok(())
}

/// Write the object map to a JSON file
fn write_object_map_json(
    output: &Path,
    map: &std::collections::BTreeMap<String, Vec<memory::ObjectMapEntry>>,
) -> Result<()> {
    let mut json = String::from("{\n");
    let class_count = map.len();
    for (i, (class_name, objects)) in map.iter().enumerate() {
        let escaped_class = class_name.replace('\\', "\\\\").replace('"', "\\\"");
        json.push_str(&format!("  \"{}\": [\n", escaped_class));
        for (j, obj) in objects.iter().enumerate() {
            let escaped_name = obj.name.replace('\\', "\\\\").replace('"', "\\\"");
            json.push_str(&format!(
                "    {{\"name\": \"{}\", \"address\": \"{:#x}\", \"class_address\": \"{:#x}\"}}",
                escaped_name, obj.address, obj.class_address
            ));
            if j < objects.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ]");
        if i < class_count - 1 {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("}\n");

    std::fs::write(output, &json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    #[test]
    fn test_find_objects_without_dump_fails() {
        let result = handle_find_objects_by_pattern("test", 10, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a memory dump"));
    }

    #[test]
    fn test_generate_object_map_without_dump_fails() {
        let result = handle_generate_object_map(None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a memory dump"));
    }

    #[test]
    fn test_write_object_map_json() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("objects.json");

        let mut map: BTreeMap<String, Vec<memory::ObjectMapEntry>> = BTreeMap::new();
        map.insert(
            "TestClass".to_string(),
            vec![
                memory::ObjectMapEntry {
                    name: "TestObject1".to_string(),
                    class_name: "TestClass".to_string(),
                    address: 0x1000,
                    class_address: 0x2000,
                },
                memory::ObjectMapEntry {
                    name: "TestObject2".to_string(),
                    class_name: "TestClass".to_string(),
                    address: 0x3000,
                    class_address: 0x2000,
                },
            ],
        );
        map.insert(
            "OtherClass".to_string(),
            vec![memory::ObjectMapEntry {
                name: "OtherObject".to_string(),
                class_name: "OtherClass".to_string(),
                address: 0x4000,
                class_address: 0x5000,
            }],
        );

        write_object_map_json(&output, &map).unwrap();

        let content = std::fs::read_to_string(&output).unwrap();
        assert!(content.contains("TestClass"));
        assert!(content.contains("TestObject1"));
        assert!(content.contains("TestObject2"));
        assert!(content.contains("OtherClass"));
        assert!(content.contains("OtherObject"));
    }

    #[test]
    fn test_write_empty_object_map() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("empty.json");

        let map: BTreeMap<String, Vec<memory::ObjectMapEntry>> = BTreeMap::new();
        write_object_map_json(&output, &map).unwrap();

        let content = std::fs::read_to_string(&output).unwrap();
        assert_eq!(content.trim(), "{\n}");
    }
}
