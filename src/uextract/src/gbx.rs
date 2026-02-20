//! Gearbox-specific native serialization deserializers
//!
//! Parses binary export data for BL4 classes that use native C++ serialization
//! instead of UE5's property system. These classes have byte 0 = 0x00 in their
//! export data (no unversioned property header).

use serde::Serialize;
use std::fmt;

/// 16-byte GUID (UE5 FGuid, MS mixed-endian format).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = &self.0;
        // MS GUID format: first 3 fields LE, last 2 fields BE
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            d[3], d[2], d[1], d[0],
            d[5], d[4],
            d[7], d[6],
            d[8], d[9],
            d[10], d[11], d[12], d[13], d[14], d[15],
        )
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({})", self)
    }
}

impl Serialize for Guid {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// -- GbxSkillParamData --

/// Parsed GbxSkillParamData export.
///
/// Contains a variant byte and a single FGuid reference to an NCS data table row.
#[derive(Debug, Clone, Serialize)]
pub struct SkillParamData {
    pub name: String,
    pub path: String,
    pub variant: u8,
    pub guid: Guid,
}

/// Parse a GbxSkillParamData export from raw bytes.
///
/// Format (27-50 bytes):
/// ```text
/// [00 02 01 03]  constant header
/// [NN]           variant byte
/// [padding...]   variable zeros
/// [16 bytes]     FGuid (always at data[len-20..len-4])
/// [00 00 00 00]  terminator
/// ```
pub fn parse_skill_param(data: &[u8], name: &str, path: &str) -> Option<SkillParamData> {
    if data.len() < 25 {
        return None;
    }

    // Validate header
    if data[0..4] != [0x00, 0x02, 0x01, 0x03] {
        return None;
    }

    let variant = data[4];

    // GUID is always at len-20..len-4, terminator is last 4 bytes
    let guid_start = data.len() - 20;
    let terminator_start = data.len() - 4;

    // Validate terminator
    if data[terminator_start..] != [0x00, 0x00, 0x00, 0x00] {
        return None;
    }

    let mut guid_bytes = [0u8; 16];
    guid_bytes.copy_from_slice(&data[guid_start..guid_start + 16]);
    let guid = Guid(guid_bytes);

    // Skip zero GUIDs (empty/default params)
    if guid.is_zero() {
        return None;
    }

    Some(SkillParamData {
        name: name.to_string(),
        path: path.to_string(),
        variant,
        guid,
    })
}

// -- GbxStatusEffectData --

/// Parsed GbxStatusEffectData export.
#[derive(Debug, Clone, Serialize)]
pub struct StatusEffectData {
    pub name: String,
    pub path: String,
    pub driver: DriverInfo,
    pub aspects: Vec<AspectInfo>,
    pub notify_events: Vec<NotifyInfo>,
    pub guids: Vec<Guid>,
    pub tags: Vec<String>,
}

/// Driver configuration for a status effect.
#[derive(Debug, Clone, Serialize)]
pub struct DriverInfo {
    pub class_name: String,
    pub strategy: Option<String>,
}

/// Aspect data — what the status effect actually does.
#[derive(Debug, Clone, Serialize)]
pub struct AspectInfo {
    pub class_name: String,
    pub guid: Option<Guid>,
    pub row_name: Option<String>,
}

/// Notify event — triggers on status effect lifecycle events.
#[derive(Debug, Clone, Serialize)]
pub struct NotifyInfo {
    pub class_name: String,
    pub event_name: String,
    pub guid: Guid,
}

/// Read a null-terminated length-prefixed FString at the given position.
///
/// Format: `LL 00 00 00 <LL bytes including null terminator>`
/// Returns (string_without_null, bytes_consumed).
fn read_fstring(data: &[u8], pos: usize) -> Option<(String, usize)> {
    if pos + 4 > data.len() {
        return None;
    }
    let len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    if len == 0 || len > 512 || pos + 4 + len > data.len() {
        return None;
    }
    // String is null-terminated; strip the null
    let end = if data[pos + 4 + len - 1] == 0 {
        pos + 4 + len - 1
    } else {
        pos + 4 + len
    };
    let s = String::from_utf8_lossy(&data[pos + 4..end]).to_string();
    Some((s, 4 + len))
}

/// Read a 16-byte GUID at the given position.
fn read_guid(data: &[u8], pos: usize) -> Option<Guid> {
    if pos + 16 > data.len() {
        return None;
    }
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&data[pos..pos + 16]);
    Some(Guid(bytes))
}

/// Scan for the next length-prefixed FString starting at or after `pos`.
///
/// Looks for plausible 4-byte LE length values followed by printable ASCII.
fn find_next_fstring(data: &[u8], start: usize) -> Option<(usize, String, usize)> {
    let mut pos = start;
    while pos + 5 < data.len() {
        let len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;

        // Plausible string: length 4-256, followed by printable ASCII, null-terminated
        if (4..=256).contains(&len)
            && pos + 4 + len <= data.len()
            && data[pos + 4].is_ascii_graphic()
            && data[pos + 4 + len - 1] == 0
        {
            let s = &data[pos + 4..pos + 4 + len - 1];
            if s.iter().all(|&b| b.is_ascii_graphic() || b == b' ' || b == b'_' || b == b'.') {
                let text = String::from_utf8_lossy(s).to_string();
                return Some((pos, text, 4 + len));
            }
        }
        pos += 1;
    }
    None
}

/// Collected status effect components during parsing.
struct StatusEffectParts {
    driver: DriverInfo,
    aspects: Vec<AspectInfo>,
    notify_events: Vec<NotifyInfo>,
    guids: Vec<Guid>,
    tags: Vec<String>,
}

/// Extract aspect data (GUID + row name) from the region after an aspect string.
fn extract_aspect(
    data: &[u8],
    text: &str,
    offset: usize,
    consumed: usize,
    next_string_end: Option<(usize, usize)>,
) -> AspectInfo {
    let search_end = next_string_end
        .map(|(pos, len)| pos + len)
        .unwrap_or(data.len());

    let mut aspect = AspectInfo {
        class_name: text.to_string(),
        guid: None,
        row_name: None,
    };

    if let Some((guid, row_name)) = find_guid_with_row_name(data, offset + consumed, search_end) {
        if !guid.is_zero() {
            aspect.guid = Some(guid);
        }
        aspect.row_name = Some(row_name);
    }

    aspect
}

/// Extract notify event data (event name + GUID) from the region after a notify string.
fn extract_notify(
    data: &[u8],
    text: &str,
    offset: usize,
    consumed: usize,
    next_string_start: Option<usize>,
) -> Option<(NotifyInfo, Guid)> {
    let search_start = offset + consumed;
    let search_end = next_string_start.unwrap_or(data.len());

    let (_, event_name, event_consumed) = find_next_fstring(data, search_start)?;
    let guid = find_guid_after(data, search_start + event_consumed, search_end)?;

    Some((
        NotifyInfo {
            class_name: text.to_string(),
            event_name,
            guid,
        },
        guid,
    ))
}

/// Classify all found FStrings into status effect components.
fn classify_status_strings(
    data: &[u8],
    all_strings: &[(usize, String, usize)],
) -> StatusEffectParts {
    let mut parts = StatusEffectParts {
        driver: DriverInfo { class_name: String::new(), strategy: None },
        aspects: Vec::new(),
        notify_events: Vec::new(),
        guids: Vec::new(),
        tags: Vec::new(),
    };

    let mut i = 0;
    while i < all_strings.len() {
        let (offset, ref text, consumed) = all_strings[i];
        let next = all_strings.get(i + 1);

        if text.starts_with("GbxStatusEffectDriver_") {
            parts.driver.class_name = text.clone();
            if let Some(next) = next.filter(|n| n.1.contains("Strategy")) {
                parts.driver.strategy = Some(next.1.clone());
                i += 2;
                continue;
            }
        } else if text.starts_with("GbxStatusEffectAspectData_") {
            let next_end = next.map(|n| (n.0, n.2));
            let aspect = extract_aspect(data, text, offset, consumed, next_end);
            if let Some(guid) = aspect.guid {
                parts.guids.push(guid);
            }
            parts.aspects.push(aspect);
        } else if text.contains("StatusEffectNotifyEventData_") || text.contains("NotifyEvent") {
            if let Some((notify, guid)) = extract_notify(data, text, offset, consumed, next.map(|n| n.0)) {
                parts.guids.push(guid);
                parts.notify_events.push(notify);
            }
        } else if text.starts_with("StatusEffects.") || text.starts_with("Gameplay.") {
            parts.tags.push(text.clone());
        } else if text.contains("OakStatusEffectMetaData") {
            for (_, ref tag_text, _) in &all_strings[i + 1..] {
                if tag_text.contains('.') && !tag_text.contains("StatusEffect") && !tag_text.starts_with('/') {
                    parts.tags.push(tag_text.clone());
                }
            }
        }

        i += 1;
    }

    parts
}

/// Parse a GbxStatusEffectData export from raw bytes.
///
/// Scans the binary data for length-prefixed type strings, extracting driver info,
/// aspects, notify events, GUIDs, and gameplay tags.
pub fn parse_status_effect(data: &[u8], name: &str, path: &str) -> Option<StatusEffectData> {
    if data.len() < 20 || data[0] != 0x00 {
        return None;
    }

    // Scan through the data finding all FStrings
    let mut pos = 0;
    let mut all_strings: Vec<(usize, String, usize)> = Vec::new();
    while let Some((offset, text, consumed)) = find_next_fstring(data, pos) {
        all_strings.push((offset, text, consumed));
        pos = offset + consumed;
    }

    let parts = classify_status_strings(data, &all_strings);

    if parts.driver.class_name.is_empty() && parts.aspects.is_empty() {
        return None;
    }

    Some(StatusEffectData {
        name: name.to_string(),
        path: path.to_string(),
        driver: parts.driver,
        aspects: parts.aspects,
        notify_events: parts.notify_events,
        guids: parts.guids,
        tags: parts.tags,
    })
}

/// Find a GUID followed by a row name string within a byte range.
///
/// Scans for 16-byte patterns that look like GUIDs (non-zero, not all-FF)
/// followed by an FString (the row name, often "Default").
fn find_guid_with_row_name(data: &[u8], start: usize, end: usize) -> Option<(Guid, String)> {
    let search_end = end.min(data.len());
    let mut pos = start;

    while pos + 16 + 5 <= search_end {
        // Check if bytes at pos look like a GUID (not all zeros, not all 0xFF)
        let candidate = &data[pos..pos + 16];
        let zeros = candidate.iter().filter(|&&b| b == 0).count();
        let ffs = candidate.iter().filter(|&&b| b == 0xFF).count();

        if zeros < 12 && ffs < 12 {
            // Check if followed by an FString
            if let Some((row_name, _)) = read_fstring(data, pos + 16) {
                if !row_name.is_empty() && row_name.len() < 64 && row_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    let guid = read_guid(data, pos)?;
                    return Some((guid, row_name));
                }
            }
        }
        pos += 1;
    }
    None
}

/// Find a GUID within a byte range (after a known position).
fn find_guid_after(data: &[u8], start: usize, end: usize) -> Option<Guid> {
    if start + 16 > end || start + 16 > data.len() {
        return None;
    }
    // The GUID is typically right at the start position
    let candidate = &data[start..start + 16];
    let zeros = candidate.iter().filter(|&&b| b == 0).count();
    if zeros < 12 {
        return read_guid(data, start);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_param_basic() {
        // Real data: SkillParam_DS_P_ConstantCompanion (27 bytes)
        let data: Vec<u8> = vec![
            0x00, 0x02, 0x01, 0x03, 0x01, 0x00, 0x00, 0xac, 0x8a, 0x04, 0xa9, 0xa0, 0xb4, 0x69,
            0x42, 0x8a, 0xf8, 0x7d, 0x87, 0x1f, 0x44, 0xef, 0x72, 0x00, 0x00, 0x00, 0x00,
        ];
        let result = parse_skill_param(&data, "SkillParam_DS_P_ConstantCompanion", "/Game/Test")
            .expect("should parse");
        assert_eq!(result.variant, 0x01);
        assert!(!result.guid.is_zero());
        assert_eq!(
            result.guid.0,
            [0xac, 0x8a, 0x04, 0xa9, 0xa0, 0xb4, 0x69, 0x42, 0x8a, 0xf8, 0x7d, 0x87, 0x1f, 0x44, 0xef, 0x72]
        );
    }

    #[test]
    fn skill_param_variant_03() {
        // Real data: SkillParam_Exo_70_BoomingBusiness_Rank (30 bytes)
        let data: Vec<u8> = vec![
            0x00, 0x02, 0x01, 0x03, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x2b, 0xe4, 0xc7,
            0xf3, 0x38, 0xb9, 0x41, 0xaf, 0xb3, 0xd8, 0xb8, 0xd8, 0xb6, 0x3c, 0x6f, 0x00, 0x00,
            0x00, 0x00,
        ];
        let result = parse_skill_param(&data, "Test", "/Game/Test").expect("should parse");
        assert_eq!(result.variant, 0x03);
        assert_eq!(
            result.guid.0,
            [0x2c, 0x2b, 0xe4, 0xc7, 0xf3, 0x38, 0xb9, 0x41, 0xaf, 0xb3, 0xd8, 0xb8, 0xd8, 0xb6, 0x3c, 0x6f]
        );
    }

    #[test]
    fn skill_param_rejects_bad_header() {
        let mut data: Vec<u8> = vec![0x01, 0x01, 0x00, 0x00, 0x00];
        data.resize(27, 0x00);
        assert!(parse_skill_param(&data, "Test", "/Test").is_none());
    }

    #[test]
    fn skill_param_rejects_short_data() {
        let data: Vec<u8> = vec![0x00, 0x02, 0x01, 0x03];
        assert!(parse_skill_param(&data, "Test", "/Test").is_none());
    }

    #[test]
    fn status_effect_firmware_buff() {
        // Real data: StatusEffect_BulletsToSpare_3 (338 bytes, truncated to driver+first aspect)
        let data: Vec<u8> = vec![
            0x00, 0x05, 0x24, 0x00, 0x00, 0x00,
            // "GbxStatusEffectDriver_InstanceStack\0" (36 bytes)
            0x47, 0x62, 0x78, 0x53, 0x74, 0x61, 0x74, 0x75, 0x73, 0x45, 0x66, 0x66, 0x65, 0x63,
            0x74, 0x44, 0x72, 0x69, 0x76, 0x65, 0x72, 0x5f, 0x49, 0x6e, 0x73, 0x74, 0x61, 0x6e,
            0x63, 0x65, 0x53, 0x74, 0x61, 0x63, 0x6b, 0x00,
            // strategy: 00 03 32 00 00 00 "GbxStatusEffectDriverInstanceStackStrategy_Capped\0"
            0x00, 0x03, 0x32, 0x00, 0x00, 0x00,
            0x47, 0x62, 0x78, 0x53, 0x74, 0x61, 0x74, 0x75, 0x73, 0x45, 0x66, 0x66, 0x65, 0x63,
            0x74, 0x44, 0x72, 0x69, 0x76, 0x65, 0x72, 0x49, 0x6e, 0x73, 0x74, 0x61, 0x6e, 0x63,
            0x65, 0x53, 0x74, 0x61, 0x63, 0x6b, 0x53, 0x74, 0x72, 0x61, 0x74, 0x65, 0x67, 0x79,
            0x5f, 0x43, 0x61, 0x70, 0x70, 0x65, 0x64, 0x00,
            // config flags + aspect start
            0x80, 0x0d, 0x38, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x09, 0x00, 0x00, 0x80, 0x3f,
            0x80, 0x07, 0x06, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf9, 0xff, 0xff,
            0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x00, 0x00, 0xf0, 0x00,
            0x00, 0x80, 0x3f,
            // aspect: 01 00 00 00 2a 00 00 00 "GbxStatusEffectAspectData_AttributeEffect\0"
            0x01, 0x00, 0x00, 0x00,
            0x2a, 0x00, 0x00, 0x00,
            0x47, 0x62, 0x78, 0x53, 0x74, 0x61, 0x74, 0x75, 0x73, 0x45, 0x66, 0x66, 0x65, 0x63,
            0x74, 0x41, 0x73, 0x70, 0x65, 0x63, 0x74, 0x44, 0x61, 0x74, 0x61, 0x5f, 0x41, 0x74,
            0x74, 0x72, 0x69, 0x62, 0x75, 0x74, 0x65, 0x45, 0x66, 0x66, 0x65, 0x63, 0x74, 0x00,
            // aspect data + GUID + "Default"
            0x00, 0x04, 0x81, 0x09, 0x01, 0x00, 0x07, 0x01, 0x00, 0x00, 0x00,
            0x80, 0x07, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x00, 0x00,
            0xf0, 0x80, 0x09, 0x01, 0x80, 0x07, 0x06, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0xf9, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0,
            0x00, 0x00, 0xf0, 0x00, 0x00, 0x80, 0x3f,
            // GUID
            0x6c, 0x00, 0xf4, 0x1a, 0x23, 0x4e, 0xda, 0x45, 0x9a, 0x58, 0x61, 0x39, 0x9f, 0x56,
            0xff, 0xaa,
            // "Default\0"
            0x08, 0x00, 0x00, 0x00, 0x44, 0x65, 0x66, 0x61, 0x75, 0x6c, 0x74, 0x00,
            // tail
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
            // more GUID
            0x4d, 0x4d, 0x38, 0x71, 0x02, 0x50, 0xd3, 0x48, 0x8e, 0x8a, 0x82, 0xe2, 0x98, 0x09,
            0x33, 0x1b,
            // tail
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xf8, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x01,
        ];

        let result = parse_status_effect(&data, "StatusEffect_BulletsToSpare_3", "/Game/Test")
            .expect("should parse");
        assert_eq!(result.driver.class_name, "GbxStatusEffectDriver_InstanceStack");
        assert_eq!(
            result.driver.strategy.as_deref(),
            Some("GbxStatusEffectDriverInstanceStackStrategy_Capped")
        );
        assert_eq!(result.aspects.len(), 1);
        assert_eq!(
            result.aspects[0].class_name,
            "GbxStatusEffectAspectData_AttributeEffect"
        );
        assert!(result.aspects[0].guid.is_some());
        assert_eq!(result.aspects[0].row_name.as_deref(), Some("Default"));
        assert!(!result.guids.is_empty());
    }

    #[test]
    fn status_effect_rejects_property_header() {
        // Byte 0 = 0x01 means UE5 unversioned properties, not Gbx native
        let mut data: Vec<u8> = vec![0x01, 0x01, 0x00, 0x00];
        data.resize(100, 0x00);
        assert!(parse_status_effect(&data, "Test", "/Test").is_none());
    }

    #[test]
    fn guid_display() {
        let guid = Guid([
            0x6c, 0x00, 0xf4, 0x1a, 0x23, 0x4e, 0xda, 0x45, 0x9a, 0x58, 0x61, 0x39, 0x9f, 0x56,
            0xff, 0xaa,
        ]);
        let s = guid.to_string();
        assert_eq!(s, "1af4006c-4e23-45da-9a58-61399f56ffaa");
    }
}
