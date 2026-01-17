//! Save file parsing, querying, and modification.
//!
//! This module provides high-level APIs for working with Borderlands 4 save files.

mod changeset;
mod state_flags;

pub use changeset::ChangeSet;
pub use state_flags::StateFlags;

use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SaveError {
    #[error("Failed to parse YAML: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Array index out of bounds: {0}")]
    IndexOutOfBounds(usize),

    #[error("Invalid array index: {0}")]
    InvalidIndex(String),
}

/// Represents a loaded save file with query/modify capabilities
pub struct SaveFile {
    data: serde_yaml::Value,
}

impl SaveFile {
    /// Parse a save file from decrypted YAML data
    pub fn from_yaml(yaml_data: &[u8]) -> Result<Self, SaveError> {
        let data = serde_yaml::from_slice(yaml_data)?;
        Ok(SaveFile { data })
    }

    /// Serialize the save file back to YAML
    pub fn to_yaml(&self) -> Result<Vec<u8>, SaveError> {
        let yaml_string = serde_yaml::to_string(&self.data)?;
        Ok(yaml_string.into_bytes())
    }

    /// Query a value at a YAML path (e.g. "state.currencies.cash" or "state.experience\[0\].level")
    pub fn get(&self, path: &str) -> Result<&serde_yaml::Value, SaveError> {
        query_yaml_path(&self.data, path)
    }

    /// Set a value at a YAML path
    pub fn set(&mut self, path: &str, value: serde_yaml::Value) -> Result<(), SaveError> {
        set_yaml_path(&mut self.data, path, value)
    }

    /// Parse a string value into the appropriate YAML type (number, bool, or string)
    pub fn parse_value(value_str: &str) -> serde_yaml::Value {
        parse_value(value_str)
    }

    /// Set a raw YAML value from a string (for complex/unknown structures)
    ///
    /// This parses a YAML string and sets it at the given path. Useful for
    /// setting complex nested structures or values we haven't decoded yet.
    ///
    /// # Example
    /// ```no_run
    /// # use bl4::SaveFile;
    /// # let mut save = SaveFile::from_yaml(&[]).unwrap();
    /// // Set a complex nested structure
    /// save.set_raw("some.unknown.field", r#"
    ///   nested:
    ///     value: 123
    ///     array: [1, 2, 3]
    /// "#).unwrap();
    /// ```
    pub fn set_raw(&mut self, path: &str, yaml_str: &str) -> Result<(), SaveError> {
        let value: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
        self.set(path, value)
    }

    /// Get character name
    pub fn get_character_name(&self) -> Option<&str> {
        self.data
            .get("state")
            .and_then(|s| s.get("char_name"))
            .and_then(|v| v.as_str())
    }

    /// Set character name
    pub fn set_character_name(&mut self, name: &str) -> Result<(), SaveError> {
        self.set(
            "state.char_name",
            serde_yaml::Value::String(name.to_string()),
        )
    }

    /// Get character class
    pub fn get_character_class(&self) -> Option<&str> {
        self.data
            .get("state")
            .and_then(|s| s.get("class"))
            .and_then(|v| v.as_str())
    }

    /// Get player difficulty
    pub fn get_difficulty(&self) -> Option<&str> {
        self.data
            .get("state")
            .and_then(|s| s.get("player_difficulty"))
            .and_then(|v| v.as_str())
    }

    /// Get cash amount
    pub fn get_cash(&self) -> Option<u64> {
        self.data
            .get("state")
            .and_then(|s| s.get("currencies"))
            .and_then(|c| c.get("cash"))
            .and_then(|v| v.as_u64())
    }

    /// Set cash amount
    pub fn set_cash(&mut self, amount: u64) -> Result<(), SaveError> {
        self.set(
            "state.currencies.cash",
            serde_yaml::Value::Number(amount.into()),
        )
    }

    /// Get eridium amount
    pub fn get_eridium(&self) -> Option<u64> {
        self.data
            .get("state")
            .and_then(|s| s.get("currencies"))
            .and_then(|c| c.get("eridium"))
            .and_then(|v| v.as_u64())
    }

    /// Set eridium amount
    pub fn set_eridium(&mut self, amount: u64) -> Result<(), SaveError> {
        self.set(
            "state.currencies.eridium",
            serde_yaml::Value::Number(amount.into()),
        )
    }

    /// Get character level and XP
    pub fn get_character_level(&self) -> Option<(u64, u64)> {
        self.data
            .get("state")
            .and_then(|s| s.get("experience"))
            .and_then(|e| e.as_sequence())
            .and_then(|arr| arr.first())
            .and_then(|exp| {
                let level = exp.get("level")?.as_u64()?;
                let points = exp.get("points")?.as_u64()?;
                Some((level, points))
            })
    }

    /// Set character XP (level is calculated from XP)
    pub fn set_character_xp(&mut self, xp: u64) -> Result<(), SaveError> {
        self.set(
            "state.experience[0].points",
            serde_yaml::Value::Number(xp.into()),
        )
    }

    /// Get specialization level and XP
    pub fn get_specialization_level(&self) -> Option<(u64, u64)> {
        self.data
            .get("state")
            .and_then(|s| s.get("experience"))
            .and_then(|e| e.as_sequence())
            .and_then(|arr| arr.get(1))
            .and_then(|exp| {
                let level = exp.get("level")?.as_u64()?;
                let points = exp.get("points")?.as_u64()?;
                Some((level, points))
            })
    }

    /// Set specialization XP (level is calculated from XP)
    pub fn set_specialization_xp(&mut self, xp: u64) -> Result<(), SaveError> {
        self.set(
            "state.experience[1].points",
            serde_yaml::Value::Number(xp.into()),
        )
    }
}

impl fmt::Debug for SaveFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SaveFile")
            .field("character_name", &self.get_character_name())
            .field("character_class", &self.get_character_class())
            .field("difficulty", &self.get_difficulty())
            .field("cash", &self.get_cash())
            .field("eridium", &self.get_eridium())
            .field("character_level", &self.get_character_level())
            .field("specialization_level", &self.get_specialization_level())
            .finish()
    }
}

// Internal helper functions

fn query_yaml_path<'a>(
    value: &'a serde_yaml::Value,
    path: &str,
) -> Result<&'a serde_yaml::Value, SaveError> {
    let mut current = value;

    for part in path.split('.') {
        // Check if this part has an array index like "experience[0]"
        if let Some(bracket_pos) = part.find('[') {
            let key = &part[..bracket_pos];
            let index_str = &part[bracket_pos + 1..part.len() - 1];
            let index: usize = index_str
                .parse()
                .map_err(|_| SaveError::InvalidIndex(index_str.to_string()))?;

            current = current
                .get(key)
                .ok_or_else(|| SaveError::KeyNotFound(key.to_string()))?;

            current = current
                .get(index)
                .ok_or(SaveError::IndexOutOfBounds(index))?;
        } else {
            current = current
                .get(part)
                .ok_or_else(|| SaveError::KeyNotFound(part.to_string()))?;
        }
    }

    Ok(current)
}

fn set_yaml_path(
    value: &mut serde_yaml::Value,
    path: &str,
    new_value: serde_yaml::Value,
) -> Result<(), SaveError> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    // Navigate to the parent of the target
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if let Some(bracket_pos) = part.find('[') {
            let key = &part[..bracket_pos];
            let index_str = &part[bracket_pos + 1..part.len() - 1];
            let index: usize = index_str
                .parse()
                .map_err(|_| SaveError::InvalidIndex(index_str.to_string()))?;

            current = current
                .get_mut(key)
                .ok_or_else(|| SaveError::KeyNotFound(key.to_string()))?;

            if is_last {
                *current
                    .get_mut(index)
                    .ok_or(SaveError::IndexOutOfBounds(index))? = new_value;
                return Ok(());
            } else {
                current = current
                    .get_mut(index)
                    .ok_or(SaveError::IndexOutOfBounds(index))?;
            }
        } else if is_last {
            *current
                .get_mut(part)
                .ok_or_else(|| SaveError::KeyNotFound(part.to_string()))? = new_value;
            return Ok(());
        } else {
            current = current
                .get_mut(part)
                .ok_or_else(|| SaveError::KeyNotFound(part.to_string()))?;
        }
    }

    Ok(())
}

pub(crate) fn parse_value(value_str: &str) -> serde_yaml::Value {
    // Try to parse as number first
    if let Ok(num) = value_str.parse::<i64>() {
        return serde_yaml::Value::Number(num.into());
    }
    if let Ok(num) = value_str.parse::<u64>() {
        return serde_yaml::Value::Number(num.into());
    }
    if let Ok(num) = value_str.parse::<f64>() {
        return serde_yaml::Value::Number(serde_yaml::Number::from(num));
    }

    // Try boolean
    if value_str.eq_ignore_ascii_case("true") {
        return serde_yaml::Value::Bool(true);
    }
    if value_str.eq_ignore_ascii_case("false") {
        return serde_yaml::Value::Bool(false);
    }

    // Default to string
    serde_yaml::Value::String(value_str.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test fixture: minimal save file YAML
    fn test_save_yaml() -> &'static str {
        r#"
state:
  char_name: TestChar
  class: Char_TestClass
  player_difficulty: Normal
  currencies:
    cash: 1000
    eridium: 50
    golden_key: shift
  experience:
    - type: Character
      level: 10
      points: 5000
    - type: Specialization
      level: 5
      points: 2500
  inventory:
    items:
      backpack:
        slot_0:
          serial: "@Test123"
          flags: 1
save_game_header:
  guid: ABC123
"#
    }

    #[test]
    fn test_save_file_from_yaml() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        assert_eq!(save.get_character_name(), Some("TestChar"));
    }

    #[test]
    fn test_query_simple_path() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let name = save.get("state.char_name").unwrap();
        assert_eq!(name.as_str(), Some("TestChar"));
    }

    #[test]
    fn test_query_nested_path() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let cash = save.get("state.currencies.cash").unwrap();
        assert_eq!(cash.as_u64(), Some(1000));
    }

    #[test]
    fn test_query_array_index() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let level = save.get("state.experience[0].level").unwrap();
        assert_eq!(level.as_u64(), Some(10));

        let spec_level = save.get("state.experience[1].level").unwrap();
        assert_eq!(spec_level.as_u64(), Some(5));
    }

    #[test]
    fn test_query_invalid_path() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let result = save.get("state.nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_query_invalid_array_index() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let result = save.get("state.experience[99]");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_simple_value() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        save.set(
            "state.char_name",
            serde_yaml::Value::String("NewName".into()),
        )
        .unwrap();
        assert_eq!(save.get_character_name(), Some("NewName"));
    }

    #[test]
    fn test_set_nested_value() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        save.set(
            "state.currencies.cash",
            serde_yaml::Value::Number(9999.into()),
        )
        .unwrap();
        assert_eq!(save.get_cash(), Some(9999));
    }

    #[test]
    fn test_set_array_element() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        save.set(
            "state.experience[0].points",
            serde_yaml::Value::Number(99999.into()),
        )
        .unwrap();
        let points = save.get("state.experience[0].points").unwrap();
        assert_eq!(points.as_u64(), Some(99999));
    }

    #[test]
    fn test_parse_value_integer() {
        let val = SaveFile::parse_value("123");
        assert_eq!(val.as_u64(), Some(123));
    }

    #[test]
    fn test_parse_value_string() {
        let val = SaveFile::parse_value("hello");
        assert_eq!(val.as_str(), Some("hello"));
    }

    #[test]
    fn test_parse_value_bool() {
        let val_true = SaveFile::parse_value("true");
        assert_eq!(val_true.as_bool(), Some(true));

        let val_false = SaveFile::parse_value("FALSE");
        assert_eq!(val_false.as_bool(), Some(false));
    }

    #[test]
    fn test_set_raw_yaml() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        save.set_raw(
            "state.currencies",
            r#"
cash: 5555
eridium: 6666
"#,
        )
        .unwrap();
        assert_eq!(save.get_cash(), Some(5555));
        assert_eq!(save.get_eridium(), Some(6666));
    }

    #[test]
    fn test_convenience_methods() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();

        assert_eq!(save.get_character_name(), Some("TestChar"));
        assert_eq!(save.get_character_class(), Some("Char_TestClass"));
        assert_eq!(save.get_difficulty(), Some("Normal"));
        assert_eq!(save.get_cash(), Some(1000));
        assert_eq!(save.get_eridium(), Some(50));
        assert_eq!(save.get_character_level(), Some((10, 5000)));
        assert_eq!(save.get_specialization_level(), Some((5, 2500)));
    }

    #[test]
    fn test_set_convenience_methods() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();

        save.set_character_name("NewChar").unwrap();
        assert_eq!(save.get_character_name(), Some("NewChar"));

        save.set_cash(77777).unwrap();
        assert_eq!(save.get_cash(), Some(77777));

        save.set_eridium(88888).unwrap();
        assert_eq!(save.get_eridium(), Some(88888));

        save.set_character_xp(99999).unwrap();
        assert_eq!(save.get_character_level(), Some((10, 99999)));

        save.set_specialization_xp(11111).unwrap();
        assert_eq!(save.get_specialization_level(), Some((5, 11111)));
    }

    #[test]
    fn test_to_yaml_roundtrip() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let yaml = save.to_yaml().unwrap();
        let save2 = SaveFile::from_yaml(&yaml).unwrap();

        assert_eq!(save2.get_character_name(), Some("TestChar"));
        assert_eq!(save2.get_cash(), Some(1000));
    }

    #[test]
    fn test_parse_value_float() {
        let val = SaveFile::parse_value("3.14159");
        assert!(val.as_f64().is_some());
        assert!((val.as_f64().unwrap() - std::f64::consts::PI).abs() < 0.01);
    }

    #[test]
    fn test_parse_value_negative_integer() {
        let val = SaveFile::parse_value("-42");
        assert_eq!(val.as_i64(), Some(-42));
    }

    #[test]
    fn test_parse_value_large_unsigned() {
        let val = SaveFile::parse_value("9999999999999");
        assert_eq!(val.as_u64(), Some(9999999999999));
    }

    #[test]
    fn test_save_file_debug() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let debug_str = format!("{:?}", save);
        assert!(debug_str.contains("SaveFile"));
        assert!(debug_str.contains("TestChar"));
        assert!(debug_str.contains("1000")); // cash
    }

    #[test]
    fn test_save_file_invalid_yaml() {
        let result = SaveFile::from_yaml(b"invalid: yaml: :::");
        assert!(result.is_err());
    }

    #[test]
    fn test_query_invalid_array_index_format() {
        let save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let result = save.get("state.experience[abc].level");
        assert!(matches!(result, Err(SaveError::InvalidIndex(_))));
    }

    #[test]
    fn test_set_invalid_array_index_format() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let result = save.set(
            "state.experience[abc].level",
            serde_yaml::Value::Number(1.into()),
        );
        assert!(matches!(result, Err(SaveError::InvalidIndex(_))));
    }
}
