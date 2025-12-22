//! Shared types for the items database.
//!
//! These types are database-agnostic and used by all implementations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Verification status for items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Decoded,
    Screenshot,
    Verified,
}

impl Default for VerificationStatus {
    fn default() -> Self {
        Self::Unverified
    }
}

impl std::fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unverified => write!(f, "unverified"),
            Self::Decoded => write!(f, "decoded"),
            Self::Screenshot => write!(f, "screenshot"),
            Self::Verified => write!(f, "verified"),
        }
    }
}

impl std::str::FromStr for VerificationStatus {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unverified" => Ok(Self::Unverified),
            "decoded" => Ok(Self::Decoded),
            "screenshot" => Ok(Self::Screenshot),
            "verified" => Ok(Self::Verified),
            _ => Err(ParseError::InvalidVerificationStatus(s.to_string())),
        }
    }
}

/// Source of a field value
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ValueSource {
    /// Value shown in the game UI (highest priority)
    InGame = 3,
    /// Value extracted by our decoder
    Decoder = 2,
    /// Value from a community tool (with source_detail naming it)
    CommunityTool = 1,
}

impl ValueSource {
    /// Priority for sorting (higher = prefer)
    pub fn priority(&self) -> u8 {
        *self as u8
    }
}

impl Default for ValueSource {
    fn default() -> Self {
        Self::Decoder
    }
}

impl std::fmt::Display for ValueSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InGame => write!(f, "ingame"),
            Self::Decoder => write!(f, "decoder"),
            Self::CommunityTool => write!(f, "community_tool"),
        }
    }
}

impl std::str::FromStr for ValueSource {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ingame" | "in_game" => Ok(Self::InGame),
            "decoder" => Ok(Self::Decoder),
            "community_tool" | "community" => Ok(Self::CommunityTool),
            _ => Err(ParseError::InvalidValueSource(s.to_string())),
        }
    }
}

/// Confidence level for a value
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Value has been verified (e.g., screenshot match)
    Verified = 3,
    /// Value is inferred but likely correct
    Inferred = 2,
    /// Value is uncertain/experimental
    Uncertain = 1,
}

impl Confidence {
    /// Priority for sorting (higher = prefer)
    pub fn priority(&self) -> u8 {
        *self as u8
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self::Inferred
    }
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Verified => write!(f, "verified"),
            Self::Inferred => write!(f, "inferred"),
            Self::Uncertain => write!(f, "uncertain"),
        }
    }
}

impl std::str::FromStr for Confidence {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "verified" => Ok(Self::Verified),
            "inferred" => Ok(Self::Inferred),
            "uncertain" => Ok(Self::Uncertain),
            _ => Err(ParseError::InvalidConfidence(s.to_string())),
        }
    }
}

/// Item fields that can have multi-source values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemField {
    Name,
    Prefix,
    Manufacturer,
    WeaponType,
    ItemType,
    Rarity,
    Level,
    Element,
    Dps,
    Damage,
    Accuracy,
    FireRate,
    ReloadTime,
    MagSize,
    Value,
    RedText,
}

impl ItemField {
    /// All field variants
    pub const ALL: &'static [ItemField] = &[
        ItemField::Name,
        ItemField::Prefix,
        ItemField::Manufacturer,
        ItemField::WeaponType,
        ItemField::ItemType,
        ItemField::Rarity,
        ItemField::Level,
        ItemField::Element,
        ItemField::Dps,
        ItemField::Damage,
        ItemField::Accuracy,
        ItemField::FireRate,
        ItemField::ReloadTime,
        ItemField::MagSize,
        ItemField::Value,
        ItemField::RedText,
    ];

    /// Display width for table formatting
    pub fn display_width(&self) -> usize {
        match self {
            Self::Name => 20,
            Self::Prefix => 15,
            Self::Manufacturer => 12,
            Self::WeaponType => 8,
            Self::ItemType => 6,
            Self::Rarity => 10,
            Self::Level => 5,
            Self::Element => 10,
            Self::Dps => 6,
            Self::Damage => 6,
            Self::Accuracy => 8,
            Self::FireRate => 10,
            Self::ReloadTime => 11,
            Self::MagSize => 8,
            Self::Value => 8,
            Self::RedText => 30,
        }
    }
}

impl std::fmt::Display for ItemField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name => write!(f, "name"),
            Self::Prefix => write!(f, "prefix"),
            Self::Manufacturer => write!(f, "manufacturer"),
            Self::WeaponType => write!(f, "weapon_type"),
            Self::ItemType => write!(f, "item_type"),
            Self::Rarity => write!(f, "rarity"),
            Self::Level => write!(f, "level"),
            Self::Element => write!(f, "element"),
            Self::Dps => write!(f, "dps"),
            Self::Damage => write!(f, "damage"),
            Self::Accuracy => write!(f, "accuracy"),
            Self::FireRate => write!(f, "fire_rate"),
            Self::ReloadTime => write!(f, "reload_time"),
            Self::MagSize => write!(f, "mag_size"),
            Self::Value => write!(f, "value"),
            Self::RedText => write!(f, "red_text"),
        }
    }
}

impl std::str::FromStr for ItemField {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "name" => Ok(Self::Name),
            "prefix" => Ok(Self::Prefix),
            "manufacturer" => Ok(Self::Manufacturer),
            "weapon_type" => Ok(Self::WeaponType),
            "item_type" => Ok(Self::ItemType),
            "rarity" => Ok(Self::Rarity),
            "level" => Ok(Self::Level),
            "element" => Ok(Self::Element),
            "dps" => Ok(Self::Dps),
            "damage" => Ok(Self::Damage),
            "accuracy" => Ok(Self::Accuracy),
            "fire_rate" => Ok(Self::FireRate),
            "reload_time" => Ok(Self::ReloadTime),
            "mag_size" => Ok(Self::MagSize),
            "value" => Ok(Self::Value),
            "red_text" => Ok(Self::RedText),
            _ => Err(ParseError::InvalidItemField(s.to_string())),
        }
    }
}

/// A field value with source attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemValue {
    pub id: i64,
    pub item_serial: String,
    pub field: String,
    pub value: String,
    pub source: ValueSource,
    pub source_detail: Option<String>,
    pub confidence: Confidence,
    pub created_at: String,
}

/// Item entry in the database (serial is the primary key)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Item {
    pub serial: String,
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub item_type: Option<String>,
    pub rarity: Option<String>,
    pub level: Option<i32>,
    pub element: Option<String>,
    pub dps: Option<i32>,
    pub damage: Option<i32>,
    pub accuracy: Option<i32>,
    pub fire_rate: Option<f64>,
    pub reload_time: Option<f64>,
    pub mag_size: Option<i32>,
    pub value: Option<i32>,
    pub red_text: Option<String>,
    pub notes: Option<String>,
    pub verification_status: VerificationStatus,
    pub verification_notes: Option<String>,
    pub verified_at: Option<String>,
    pub legal: bool,
    pub source: Option<String>,
    pub created_at: String,
}

/// Weapon part entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPart {
    pub id: i64,
    pub item_serial: String,
    pub slot: String,
    pub part_index: Option<i32>,
    pub part_name: Option<String>,
    pub manufacturer: Option<String>,
    pub effect: Option<String>,
    pub verified: bool,
    pub verification_method: Option<String>,
    pub verification_notes: Option<String>,
    pub verified_at: Option<String>,
}

/// Image attachment entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: i64,
    pub item_serial: String,
    pub name: String,
    pub mime_type: String,
    /// View type: POPUP (item card), DETAIL (3D inspect), or OTHER
    pub view: String,
}

/// Database statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DbStats {
    pub item_count: i64,
    pub part_count: i64,
    pub attachment_count: i64,
    pub value_count: i64,
}

/// Migration statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationStats {
    pub items_processed: usize,
    pub values_migrated: usize,
    pub values_skipped: usize,
}

/// Filter for listing items
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ItemFilter {
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub element: Option<String>,
    pub rarity: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Update payload for items
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ItemUpdate {
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub rarity: Option<String>,
    pub level: Option<i32>,
    pub element: Option<String>,
    pub dps: Option<i32>,
    pub damage: Option<i32>,
    pub accuracy: Option<i32>,
    pub fire_rate: Option<f64>,
    pub reload_time: Option<f64>,
    pub mag_size: Option<i32>,
    pub value: Option<i32>,
    pub red_text: Option<String>,
    pub notes: Option<String>,
}

/// Parse errors for string conversions
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid verification status: {0}")]
    InvalidVerificationStatus(String),
    #[error("Invalid value source: {0}")]
    InvalidValueSource(String),
    #[error("Invalid confidence level: {0}")]
    InvalidConfidence(String),
    #[error("Invalid item field: {0}")]
    InvalidItemField(String),
}

/// Helper to pick the best value from a collection based on source and confidence priority
pub fn pick_best_value(values: impl IntoIterator<Item = ItemValue>) -> Option<ItemValue> {
    values.into_iter().max_by(|a, b| {
        match a.source.priority().cmp(&b.source.priority()) {
            std::cmp::Ordering::Equal => a.confidence.priority().cmp(&b.confidence.priority()),
            other => other,
        }
    })
}

/// Group values by field and pick best for each
pub fn best_values_by_field(values: impl IntoIterator<Item = ItemValue>) -> HashMap<String, String> {
    let mut best_by_field: HashMap<String, ItemValue> = HashMap::new();

    for value in values {
        let dominated = best_by_field.get(&value.field).map(|existing| {
            match value.source.priority().cmp(&existing.source.priority()) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => value.confidence.priority() > existing.confidence.priority(),
                std::cmp::Ordering::Less => false,
            }
        }).unwrap_or(true);

        if dominated {
            best_by_field.insert(value.field.clone(), value);
        }
    }

    best_by_field.into_iter().map(|(k, v)| (k, v.value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_status_parse() {
        assert_eq!("unverified".parse::<VerificationStatus>().unwrap(), VerificationStatus::Unverified);
        assert_eq!("decoded".parse::<VerificationStatus>().unwrap(), VerificationStatus::Decoded);
        assert_eq!("screenshot".parse::<VerificationStatus>().unwrap(), VerificationStatus::Screenshot);
        assert_eq!("verified".parse::<VerificationStatus>().unwrap(), VerificationStatus::Verified);
        assert!("invalid".parse::<VerificationStatus>().is_err());
    }

    #[test]
    fn test_verification_status_display() {
        assert_eq!(VerificationStatus::Unverified.to_string(), "unverified");
        assert_eq!(VerificationStatus::Decoded.to_string(), "decoded");
        assert_eq!(VerificationStatus::Screenshot.to_string(), "screenshot");
        assert_eq!(VerificationStatus::Verified.to_string(), "verified");
    }

    #[test]
    fn test_value_source_parse() {
        assert_eq!("ingame".parse::<ValueSource>().unwrap(), ValueSource::InGame);
        assert_eq!("in_game".parse::<ValueSource>().unwrap(), ValueSource::InGame);
        assert_eq!("decoder".parse::<ValueSource>().unwrap(), ValueSource::Decoder);
        assert_eq!("community_tool".parse::<ValueSource>().unwrap(), ValueSource::CommunityTool);
        assert_eq!("community".parse::<ValueSource>().unwrap(), ValueSource::CommunityTool);
        assert!("invalid".parse::<ValueSource>().is_err());
    }

    #[test]
    fn test_value_source_display() {
        assert_eq!(ValueSource::InGame.to_string(), "ingame");
        assert_eq!(ValueSource::Decoder.to_string(), "decoder");
        assert_eq!(ValueSource::CommunityTool.to_string(), "community_tool");
    }

    #[test]
    fn test_value_source_priority() {
        assert!(ValueSource::InGame.priority() > ValueSource::Decoder.priority());
        assert!(ValueSource::Decoder.priority() > ValueSource::CommunityTool.priority());
    }

    #[test]
    fn test_confidence_parse() {
        assert_eq!("verified".parse::<Confidence>().unwrap(), Confidence::Verified);
        assert_eq!("inferred".parse::<Confidence>().unwrap(), Confidence::Inferred);
        assert_eq!("uncertain".parse::<Confidence>().unwrap(), Confidence::Uncertain);
        assert!("invalid".parse::<Confidence>().is_err());
    }

    #[test]
    fn test_confidence_display() {
        assert_eq!(Confidence::Verified.to_string(), "verified");
        assert_eq!(Confidence::Inferred.to_string(), "inferred");
        assert_eq!(Confidence::Uncertain.to_string(), "uncertain");
    }

    #[test]
    fn test_confidence_priority() {
        assert!(Confidence::Verified.priority() > Confidence::Inferred.priority());
        assert!(Confidence::Inferred.priority() > Confidence::Uncertain.priority());
    }

    #[test]
    fn test_item_field_parse() {
        assert_eq!("name".parse::<ItemField>().unwrap(), ItemField::Name);
        assert_eq!("prefix".parse::<ItemField>().unwrap(), ItemField::Prefix);
        assert_eq!("manufacturer".parse::<ItemField>().unwrap(), ItemField::Manufacturer);
        assert_eq!("weapon_type".parse::<ItemField>().unwrap(), ItemField::WeaponType);
        assert_eq!("item_type".parse::<ItemField>().unwrap(), ItemField::ItemType);
        assert_eq!("rarity".parse::<ItemField>().unwrap(), ItemField::Rarity);
        assert_eq!("level".parse::<ItemField>().unwrap(), ItemField::Level);
        assert_eq!("element".parse::<ItemField>().unwrap(), ItemField::Element);
        assert!("invalid".parse::<ItemField>().is_err());
    }

    #[test]
    fn test_item_field_display() {
        assert_eq!(ItemField::Name.to_string(), "name");
        assert_eq!(ItemField::WeaponType.to_string(), "weapon_type");
    }

    fn make_value(field: &str, value: &str, source: ValueSource, confidence: Confidence) -> ItemValue {
        ItemValue {
            id: 0,
            item_serial: String::new(),
            field: field.to_string(),
            value: value.to_string(),
            source,
            source_detail: None,
            confidence,
            created_at: String::new(),
        }
    }

    #[test]
    fn test_pick_best_value_by_source() {
        let values = vec![
            make_value("name", "Community Name", ValueSource::CommunityTool, Confidence::Verified),
            make_value("name", "Decoder Name", ValueSource::Decoder, Confidence::Verified),
            make_value("name", "InGame Name", ValueSource::InGame, Confidence::Verified),
        ];
        let best = pick_best_value(values).unwrap();
        assert_eq!(best.value, "InGame Name");
        assert_eq!(best.source, ValueSource::InGame);
    }

    #[test]
    fn test_pick_best_value_by_confidence() {
        let values = vec![
            make_value("name", "Uncertain", ValueSource::Decoder, Confidence::Uncertain),
            make_value("name", "Verified", ValueSource::Decoder, Confidence::Verified),
            make_value("name", "Inferred", ValueSource::Decoder, Confidence::Inferred),
        ];
        let best = pick_best_value(values).unwrap();
        assert_eq!(best.value, "Verified");
        assert_eq!(best.confidence, Confidence::Verified);
    }

    #[test]
    fn test_pick_best_value_source_over_confidence() {
        // InGame with Uncertain should beat Decoder with Verified
        let values = vec![
            make_value("name", "Decoder Verified", ValueSource::Decoder, Confidence::Verified),
            make_value("name", "InGame Uncertain", ValueSource::InGame, Confidence::Uncertain),
        ];
        let best = pick_best_value(values).unwrap();
        assert_eq!(best.value, "InGame Uncertain");
    }

    #[test]
    fn test_pick_best_value_empty() {
        let values: Vec<ItemValue> = vec![];
        assert!(pick_best_value(values).is_none());
    }

    #[test]
    fn test_best_values_by_field() {
        let values = vec![
            make_value("name", "Bad Name", ValueSource::CommunityTool, Confidence::Uncertain),
            make_value("name", "Good Name", ValueSource::InGame, Confidence::Verified),
            make_value("level", "50", ValueSource::Decoder, Confidence::Inferred),
            make_value("level", "51", ValueSource::InGame, Confidence::Verified),
        ];
        let best = best_values_by_field(values);
        assert_eq!(best.get("name"), Some(&"Good Name".to_string()));
        assert_eq!(best.get("level"), Some(&"51".to_string()));
    }

    #[test]
    fn test_best_values_by_field_empty() {
        let values: Vec<ItemValue> = vec![];
        let best = best_values_by_field(values);
        assert!(best.is_empty());
    }
}
