//! Fog of Discovery (FOD) manipulation for save files.
//!
//! FOD data represents the map exploration state: a 128x128 grayscale grid
//! per zone, where 0 = fogged and 255 = fully revealed.

use base64::prelude::*;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;

use super::SaveError;

const FOD_GRID_SIZE: usize = 128 * 128;

/// Generate a FOD payload filled with `value` (base64-encoded zlib-compressed grid).
fn filled_foddata(value: u8) -> Result<String, SaveError> {
    let grid = vec![value; FOD_GRID_SIZE];
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&grid)
        .map_err(|e| SaveError::FodCompress(e.to_string()))?;
    let compressed = encoder
        .finish()
        .map_err(|e| SaveError::FodCompress(e.to_string()))?;
    Ok(BASE64_STANDARD.encode(compressed))
}

/// Replace FOD data for matching zones with a given fill value.
///
/// If `zone` is Some, only affects that zone. Otherwise affects all zones.
/// Returns the number of zones modified.
fn fill_map(
    data: &mut serde_yaml::Value,
    zone: Option<&str>,
    fill: u8,
) -> Result<usize, SaveError> {
    // In real saves, foddatas lives under gbx_discovery_pc; fall back to root.
    let has_discovery = data.get("gbx_discovery_pc").is_some();
    let container = if has_discovery {
        data.get_mut("gbx_discovery_pc").unwrap()
    } else {
        data
    };
    let foddatas = container
        .get_mut("foddatas")
        .and_then(|v| v.as_sequence_mut())
        .ok_or_else(|| SaveError::KeyNotFound("foddatas".to_string()))?;

    let payload = filled_foddata(fill)?;
    let mut count = 0;

    for entry in foddatas.iter_mut() {
        let levelname = entry
            .get("levelname")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if let Some(filter) = zone {
            if !levelname.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        if let Some(field) = entry.get_mut("foddata") {
            *field = serde_yaml::Value::String(payload.clone());
            count += 1;
        }
    }

    if zone.is_some() && count == 0 {
        return Err(SaveError::KeyNotFound(format!(
            "zone '{}' not found in foddatas",
            zone.unwrap()
        )));
    }

    Ok(count)
}

/// Reveal the map (set all FOD cells to 0xFF = fully explored).
pub fn reveal_map(
    data: &mut serde_yaml::Value,
    zone: Option<&str>,
) -> Result<usize, SaveError> {
    fill_map(data, zone, 0xFF)
}

/// Clear the map (set all FOD cells to 0x00 = fully fogged).
pub fn clear_map(
    data: &mut serde_yaml::Value,
    zone: Option<&str>,
) -> Result<usize, SaveError> {
    fill_map(data, zone, 0x00)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    fn decode_foddata(encoded: &str) -> Vec<u8> {
        let compressed = BASE64_STANDARD.decode(encoded).unwrap();
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).unwrap();
        buf
    }

    fn test_save_with_fod() -> serde_yaml::Value {
        serde_yaml::from_str(
            r#"
fodsaveversion: 2
foddatas:
  - levelname: World_P
    foddimensionx: 128
    foddimensiony: 128
    compressiontype: Zlib
    foddata: "placeholder_a"
  - levelname: Fortress_Grasslands_P
    foddimensionx: 128
    foddimensiony: 128
    compressiontype: Zlib
    foddata: "placeholder_b"
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_filled_foddata_roundtrip() {
        for fill in [0x00, 0x7F, 0xFF] {
            let encoded = filled_foddata(fill).unwrap();
            let buf = decode_foddata(&encoded);
            assert_eq!(buf.len(), FOD_GRID_SIZE);
            assert!(buf.iter().all(|&b| b == fill));
        }
    }

    #[test]
    fn test_reveal_all_zones() {
        let mut data = test_save_with_fod();
        let count = reveal_map(&mut data, None).unwrap();
        assert_eq!(count, 2);

        for entry in data["foddatas"].as_sequence().unwrap() {
            let buf = decode_foddata(entry["foddata"].as_str().unwrap());
            assert_eq!(buf.len(), FOD_GRID_SIZE);
            assert!(buf.iter().all(|&b| b == 0xFF));
        }
    }

    #[test]
    fn test_clear_all_zones() {
        let mut data = test_save_with_fod();
        let count = clear_map(&mut data, None).unwrap();
        assert_eq!(count, 2);

        for entry in data["foddatas"].as_sequence().unwrap() {
            let buf = decode_foddata(entry["foddata"].as_str().unwrap());
            assert_eq!(buf.len(), FOD_GRID_SIZE);
            assert!(buf.iter().all(|&b| b == 0x00));
        }
    }

    #[test]
    fn test_reveal_single_zone() {
        let mut data = test_save_with_fod();
        let count = reveal_map(&mut data, Some("World_P")).unwrap();
        assert_eq!(count, 1);

        let foddatas = data["foddatas"].as_sequence().unwrap();
        assert_ne!(foddatas[0]["foddata"].as_str().unwrap(), "placeholder_a");
        assert_eq!(foddatas[1]["foddata"].as_str().unwrap(), "placeholder_b");
    }

    #[test]
    fn test_clear_single_zone() {
        let mut data = test_save_with_fod();
        let count = clear_map(&mut data, Some("Fortress_Grasslands_P")).unwrap();
        assert_eq!(count, 1);

        let foddatas = data["foddatas"].as_sequence().unwrap();
        assert_eq!(foddatas[0]["foddata"].as_str().unwrap(), "placeholder_a");
        assert_ne!(foddatas[1]["foddata"].as_str().unwrap(), "placeholder_b");
    }

    #[test]
    fn test_zone_not_found() {
        let mut data = test_save_with_fod();
        assert!(reveal_map(&mut data, Some("NonExistent_P")).is_err());
        assert!(clear_map(&mut data, Some("NonExistent_P")).is_err());
    }

    #[test]
    fn test_no_foddatas() {
        let mut data: serde_yaml::Value =
            serde_yaml::from_str("state: {}").unwrap();
        assert!(reveal_map(&mut data, None).is_err());
        assert!(clear_map(&mut data, None).is_err());
    }
}
