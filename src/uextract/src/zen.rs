//! Zen format parsing to JSON

use anyhow::Result;
use retoc::{
    container_header::EIoContainerHeaderVersion, zen::FZenPackageHeader, EIoStoreTocVersion,
};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use usmap::Usmap;

use crate::property::{parse_export_properties, parse_export_properties_with_schema};
use crate::types::{ZenAssetInfo, ZenExportInfo, ZenImportInfo};

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn parse_zen_to_json(
    data: &[u8],
    path: &str,
    toc_version: EIoStoreTocVersion,
    container_header_version: EIoContainerHeaderVersion,
    usmap_schema: Option<&Arc<Usmap>>,
    class_lookup: Option<&Arc<HashMap<String, String>>>,
    verbose: bool,
) -> Result<String> {
    let mut cursor = Cursor::new(data);

    let header = FZenPackageHeader::deserialize(
        &mut cursor,
        None,
        toc_version,
        container_header_version,
        None,
    )?;

    let header_end = cursor.position() as usize;
    let names: Vec<String> = header.name_map.copy_raw_names();

    let struct_lookup: HashMap<String, &usmap::Struct> = usmap_schema
        .map(|schema| schema.structs.iter().map(|s| (s.name.clone(), s)).collect())
        .unwrap_or_default();

    let imports: Vec<ZenImportInfo> = header
        .import_map
        .iter()
        .enumerate()
        .map(|(i, import)| ZenImportInfo {
            index: i,
            type_name: format!("{:?}", import),
        })
        .collect();

    let exports: Vec<ZenExportInfo> = header
        .export_map
        .iter()
        .enumerate()
        .map(|(i, export)| {
            let absolute_offset = header_end + export.cooked_serial_offset as usize;

            let resolved_class_name: Option<String> = if export.class_index.kind()
                == retoc::script_objects::FPackageObjectIndexType::ScriptImport
            {
                let class_hash = format!("{:X}", export.class_index.raw_index());
                class_lookup
                    .and_then(|lookup| lookup.get(&class_hash))
                    .map(|path| path.rsplit('.').next().unwrap_or(path).to_string())
            } else {
                None
            };

            let properties = if usmap_schema.is_some() {
                parse_export_properties_with_schema(
                    data,
                    absolute_offset,
                    export.cooked_serial_size as usize,
                    &names,
                    &struct_lookup,
                    resolved_class_name.as_deref(),
                    verbose,
                )
            } else {
                parse_export_properties(
                    data,
                    absolute_offset,
                    export.cooked_serial_size as usize,
                    &names,
                )
            };

            ZenExportInfo {
                index: i,
                object_name: header.name_map.get(export.object_name).to_string(),
                class_index: format!("{:?}", export.class_index),
                super_index: format!("{:?}", export.super_index),
                template_index: format!("{:?}", export.template_index),
                outer_index: format!("{:?}", export.outer_index),
                public_export_hash: export.public_export_hash,
                cooked_serial_offset: export.cooked_serial_offset,
                cooked_serial_size: export.cooked_serial_size,
                properties,
            }
        })
        .collect();

    let info = ZenAssetInfo {
        path: path.to_string(),
        package_name: header.package_name(),
        package_flags: header.summary.package_flags,
        is_unversioned: header.is_unversioned,
        name_count: names.len(),
        import_count: imports.len(),
        export_count: exports.len(),
        names,
        imports,
        exports,
    };

    Ok(serde_json::to_string_pretty(&info)?)
}
