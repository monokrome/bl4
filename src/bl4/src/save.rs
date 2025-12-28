//! Save file parsing, querying, and modification.
//!
//! This module provides high-level APIs for working with Borderlands 4 save files.

use serde_yaml;
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

/// State flags bitmask helper for inventory items.
///
/// Items in Borderlands 4 saves have a `state_flags` field that encodes
/// various properties using a bitmask. This struct provides a type-safe
/// way to work with these flags without knowing the bit positions.
///
/// # Example
/// ```
/// use bl4::StateFlags;
///
/// // Create flags for a backpack item marked as favorite
/// let flags = StateFlags::backpack().with_favorite();
///
/// // Create flags for an equipped item
/// let equipped = StateFlags::equipped();
///
/// // Query flags
/// assert!(flags.is_favorite());
/// assert!(flags.is_in_backpack());
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateFlags(pub u32);

impl StateFlags {
    // Bit values matching Borderlands 4's state_flags field (verified in-game)
    const VALID: u32 = 1; // bit 0 - item exists/valid
    const FAVORITE: u32 = 2; // bit 1 - favorite
    const JUNK: u32 = 4; // bit 2 - junk marker
    const LABEL1: u32 = 16; // bit 4 - label 1
    const LABEL2: u32 = 32; // bit 5 - label 2
    const LABEL3: u32 = 64; // bit 6 - label 3
    const LABEL4: u32 = 128; // bit 7 - label 4
    const IN_BACKPACK: u32 = 512; // bit 9 - in backpack (not equipped)

    // All label bits are mutually exclusive (only one can be set at a time)
    const ALL_LABELS: u32 =
        Self::FAVORITE | Self::JUNK | Self::LABEL1 | Self::LABEL2 | Self::LABEL3 | Self::LABEL4;

    /// Create flags for a backpack item (valid + in_backpack).
    pub fn backpack() -> Self {
        Self(Self::VALID | Self::IN_BACKPACK)
    }

    /// Create flags for an equipped item (valid only, no backpack bit).
    pub fn equipped() -> Self {
        Self(Self::VALID)
    }

    /// Create flags for a bank item (valid only).
    pub fn bank() -> Self {
        Self(Self::VALID)
    }

    /// Create flags from a raw u32 value.
    pub fn from_raw(bits: u32) -> Self {
        Self(bits)
    }

    /// Get the raw u32 value.
    pub fn to_raw(self) -> u32 {
        self.0
    }

    // Builder methods (chainable)
    // Note: Labels are mutually exclusive - setting one clears others

    /// Set the favorite label (clears other labels).
    pub fn with_favorite(mut self) -> Self {
        self.0 = (self.0 & !Self::ALL_LABELS) | Self::FAVORITE;
        self
    }

    /// Set the junk label (clears other labels).
    pub fn with_junk(mut self) -> Self {
        self.0 = (self.0 & !Self::ALL_LABELS) | Self::JUNK;
        self
    }

    /// Set label 1 (clears other labels).
    pub fn with_label1(mut self) -> Self {
        self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL1;
        self
    }

    /// Set label 2 (clears other labels).
    pub fn with_label2(mut self) -> Self {
        self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL2;
        self
    }

    /// Set label 3 (clears other labels).
    pub fn with_label3(mut self) -> Self {
        self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL3;
        self
    }

    /// Set label 4 (clears other labels).
    pub fn with_label4(mut self) -> Self {
        self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL4;
        self
    }

    /// Clear all labels (favorite, junk, 1-4).
    pub fn with_no_label(mut self) -> Self {
        self.0 &= !Self::ALL_LABELS;
        self
    }

    // Query methods

    /// Check if the favorite flag is set.
    pub fn is_favorite(&self) -> bool {
        self.0 & Self::FAVORITE != 0
    }

    /// Check if the junk flag is set.
    pub fn is_junk(&self) -> bool {
        self.0 & Self::JUNK != 0
    }

    /// Check if label 1 is set.
    pub fn has_label1(&self) -> bool {
        self.0 & Self::LABEL1 != 0
    }

    /// Check if label 2 is set.
    pub fn has_label2(&self) -> bool {
        self.0 & Self::LABEL2 != 0
    }

    /// Check if label 3 is set.
    pub fn has_label3(&self) -> bool {
        self.0 & Self::LABEL3 != 0
    }

    /// Check if label 4 is set.
    pub fn has_label4(&self) -> bool {
        self.0 & Self::LABEL4 != 0
    }

    /// Check if the item is in backpack (not equipped).
    pub fn is_in_backpack(&self) -> bool {
        self.0 & Self::IN_BACKPACK != 0
    }

    /// Check if the item is equipped (not in backpack only).
    pub fn is_equipped(&self) -> bool {
        !self.is_in_backpack()
    }

    // Mutation methods
    // Note: Labels are mutually exclusive - setting one clears others

    /// Set favorite label (clears other labels) or clear it.
    pub fn set_favorite(&mut self, value: bool) {
        if value {
            self.0 = (self.0 & !Self::ALL_LABELS) | Self::FAVORITE;
        } else {
            self.0 &= !Self::FAVORITE;
        }
    }

    /// Set junk label (clears other labels) or clear it.
    pub fn set_junk(&mut self, value: bool) {
        if value {
            self.0 = (self.0 & !Self::ALL_LABELS) | Self::JUNK;
        } else {
            self.0 &= !Self::JUNK;
        }
    }

    /// Set label 1 (clears other labels) or clear it.
    pub fn set_label1(&mut self, value: bool) {
        if value {
            self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL1;
        } else {
            self.0 &= !Self::LABEL1;
        }
    }

    /// Set label 2 (clears other labels) or clear it.
    pub fn set_label2(&mut self, value: bool) {
        if value {
            self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL2;
        } else {
            self.0 &= !Self::LABEL2;
        }
    }

    /// Set label 3 (clears other labels) or clear it.
    pub fn set_label3(&mut self, value: bool) {
        if value {
            self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL3;
        } else {
            self.0 &= !Self::LABEL3;
        }
    }

    /// Set label 4 (clears other labels) or clear it.
    pub fn set_label4(&mut self, value: bool) {
        if value {
            self.0 = (self.0 & !Self::ALL_LABELS) | Self::LABEL4;
        } else {
            self.0 &= !Self::LABEL4;
        }
    }

    /// Clear all labels.
    pub fn clear_labels(&mut self) {
        self.0 &= !Self::ALL_LABELS;
    }

    /// Convert to equipped flags (clear backpack bit).
    pub fn to_equipped(mut self) -> Self {
        self.0 &= !Self::IN_BACKPACK;
        self
    }

    /// Convert to backpack flags (set backpack bit).
    pub fn to_backpack(mut self) -> Self {
        self.0 |= Self::IN_BACKPACK;
        self
    }
}

impl From<u32> for StateFlags {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

impl From<StateFlags> for u32 {
    fn from(f: StateFlags) -> Self {
        f.0
    }
}

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

/// Represents a set of changes to apply to a save file
///
/// This is useful for batching multiple changes together and for
/// tracking modifications in a GUI before applying them.
#[derive(Default, Debug, Clone)]
pub struct ChangeSet {
    changes: HashMap<String, serde_yaml::Value>,
}

impl ChangeSet {
    /// Create a new empty ChangeSet
    pub fn new() -> Self {
        ChangeSet {
            changes: HashMap::new(),
        }
    }

    /// Add a change to the set
    pub fn add(&mut self, path: String, value: serde_yaml::Value) {
        self.changes.insert(path, value);
    }

    /// Add a change with a string value (auto-parsed)
    pub fn add_parsed(&mut self, path: String, value_str: &str) {
        let value = parse_value(value_str);
        self.changes.insert(path, value);
    }

    /// Add a raw YAML change from a string (for complex/unknown structures)
    pub fn add_raw(&mut self, path: String, yaml_str: &str) -> Result<(), SaveError> {
        let value: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
        self.changes.insert(path, value);
        Ok(())
    }

    /// Check if a specific path has been modified
    pub fn has_change(&self, path: &str) -> bool {
        self.changes.contains_key(path)
    }

    /// Get the pending change for a path, if any
    pub fn get_change(&self, path: &str) -> Option<&serde_yaml::Value> {
        self.changes.get(path)
    }

    /// Remove a change from the set
    pub fn remove(&mut self, path: &str) -> Option<serde_yaml::Value> {
        self.changes.remove(path)
    }

    /// Clear all changes
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// Get number of changes
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Get an iterator over all changes
    pub fn iter(&self) -> impl Iterator<Item = (&String, &serde_yaml::Value)> {
        self.changes.iter()
    }

    /// Apply all changes to a SaveFile
    pub fn apply(&self, save: &mut SaveFile) -> Result<(), SaveError> {
        for (path, value) in &self.changes {
            save.set(path, value.clone())?;
        }
        Ok(())
    }

    /// Convenience methods for common operations
    ///
    /// Set character name
    pub fn set_character_name(&mut self, name: &str) {
        self.add(
            "state.char_name".to_string(),
            serde_yaml::Value::String(name.to_string()),
        );
    }

    /// Set cash amount
    pub fn set_cash(&mut self, amount: u64) {
        self.add(
            "state.currencies.cash".to_string(),
            serde_yaml::Value::Number(amount.into()),
        );
    }

    /// Set eridium amount
    pub fn set_eridium(&mut self, amount: u64) {
        self.add(
            "state.currencies.eridium".to_string(),
            serde_yaml::Value::Number(amount.into()),
        );
    }

    /// Set character XP
    pub fn set_character_xp(&mut self, xp: u64) {
        self.add(
            "state.experience[0].points".to_string(),
            serde_yaml::Value::Number(xp.into()),
        );
    }

    /// Set specialization XP
    pub fn set_specialization_xp(&mut self, xp: u64) {
        self.add(
            "state.experience[1].points".to_string(),
            serde_yaml::Value::Number(xp.into()),
        );
    }

    // ─────────────────────────────────────────────────────────────────
    // Backpack Item Operations
    // ─────────────────────────────────────────────────────────────────

    /// Add an item to a backpack slot.
    ///
    /// # Arguments
    /// * `slot` - Backpack slot number (0-22 typically)
    /// * `serial` - Item serial string (e.g., "@Ugr$ZCm/...")
    /// * `flags` - State flags for the item
    ///
    /// # Example
    /// ```
    /// use bl4::{ChangeSet, StateFlags};
    ///
    /// let mut changes = ChangeSet::new();
    /// changes.add_backpack_item(0, "@Ugr$ZCm/...", StateFlags::backpack());
    /// ```
    pub fn add_backpack_item(&mut self, slot: u8, serial: &str, flags: StateFlags) {
        let base = format!("state.inventory.items.backpack.slot_{}", slot);
        self.add(
            format!("{}.serial", base),
            serde_yaml::Value::String(serial.to_string()),
        );
        self.add(
            format!("{}.flags", base),
            serde_yaml::Value::Number(0.into()),
        );
        self.add(
            format!("{}.state_flags", base),
            serde_yaml::Value::Number((flags.0 as i64).into()),
        );
    }

    /// Set state_flags on an existing backpack item.
    pub fn set_backpack_flags(&mut self, slot: u8, flags: StateFlags) {
        self.add(
            format!("state.inventory.items.backpack.slot_{}.state_flags", slot),
            serde_yaml::Value::Number((flags.0 as i64).into()),
        );
    }

    /// Set or clear the favorite flag on a backpack item.
    pub fn set_favorite(&mut self, slot: u8, value: bool) {
        let mut flags = StateFlags::backpack();
        flags.set_favorite(value);
        self.set_backpack_flags(slot, flags);
    }

    /// Set or clear the junk flag on a backpack item.
    pub fn set_junk(&mut self, slot: u8, value: bool) {
        let mut flags = StateFlags::backpack();
        flags.set_junk(value);
        self.set_backpack_flags(slot, flags);
    }

    /// Set or clear label 1 on a backpack item.
    pub fn set_label1(&mut self, slot: u8, value: bool) {
        let mut flags = StateFlags::backpack();
        flags.set_label1(value);
        self.set_backpack_flags(slot, flags);
    }

    /// Set or clear label 2 on a backpack item.
    pub fn set_label2(&mut self, slot: u8, value: bool) {
        let mut flags = StateFlags::backpack();
        flags.set_label2(value);
        self.set_backpack_flags(slot, flags);
    }

    /// Set or clear label 3 on a backpack item.
    pub fn set_label3(&mut self, slot: u8, value: bool) {
        let mut flags = StateFlags::backpack();
        flags.set_label3(value);
        self.set_backpack_flags(slot, flags);
    }

    /// Set or clear label 4 on a backpack item.
    pub fn set_label4(&mut self, slot: u8, value: bool) {
        let mut flags = StateFlags::backpack();
        flags.set_label4(value);
        self.set_backpack_flags(slot, flags);
    }

    // ─────────────────────────────────────────────────────────────────
    // Bank Item Operations (profile.sav)
    // ─────────────────────────────────────────────────────────────────

    /// Add an item to a bank slot.
    ///
    /// Note: Bank items are stored in profile.sav, not character saves.
    ///
    /// # Arguments
    /// * `slot` - Bank slot number
    /// * `serial` - Item serial string
    /// * `flags` - State flags for the item
    pub fn add_bank_item(&mut self, slot: u16, serial: &str, flags: StateFlags) {
        let base = format!("domains.local.shared.inventory.items.bank.slot_{}", slot);
        self.add(
            format!("{}.serial", base),
            serde_yaml::Value::String(serial.to_string()),
        );
        self.add(
            format!("{}.state_flags", base),
            serde_yaml::Value::Number((flags.0 as i64).into()),
        );
    }

    /// Set state_flags on an existing bank item.
    pub fn set_bank_flags(&mut self, slot: u16, flags: StateFlags) {
        self.add(
            format!(
                "domains.local.shared.inventory.items.bank.slot_{}.state_flags",
                slot
            ),
            serde_yaml::Value::Number((flags.0 as i64).into()),
        );
    }

    // ─────────────────────────────────────────────────────────────────
    // Equipped Item Operations
    // ─────────────────────────────────────────────────────────────────

    /// Equip an item to a slot.
    ///
    /// This adds the item to equipped_inventory. The item should also
    /// exist in the backpack with matching flags.
    ///
    /// # Arguments
    /// * `slot` - Equipped slot (0-3 weapons, 4 shield, 5 grenade, 6+ gear)
    /// * `serial` - Item serial string
    pub fn equip_item(&mut self, slot: u8, serial: &str) {
        let yaml = format!("- serial: '{}'\n  flags: 1\n  state_flags: 1", serial);
        let _ = self.add_raw(
            format!("state.inventory.equipped_inventory.equipped.slot_{}", slot),
            &yaml,
        );
    }

    /// Clear an equipped slot (unequip item).
    pub fn unequip_slot(&mut self, slot: u8) {
        let _ = self.add_raw(
            format!("state.inventory.equipped_inventory.equipped.slot_{}", slot),
            "[]",
        );
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

fn parse_value(value_str: &str) -> serde_yaml::Value {
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
    fn test_changeset_new() {
        let changeset = ChangeSet::new();
        assert!(changeset.is_empty());
        assert_eq!(changeset.len(), 0);
    }

    #[test]
    fn test_changeset_add() {
        let mut changeset = ChangeSet::new();
        changeset.add(
            "state.cash".to_string(),
            serde_yaml::Value::Number(999.into()),
        );

        assert_eq!(changeset.len(), 1);
        assert!(changeset.has_change("state.cash"));
        assert!(!changeset.has_change("state.eridium"));
    }

    #[test]
    fn test_changeset_add_parsed() {
        let mut changeset = ChangeSet::new();
        changeset.add_parsed("state.cash".to_string(), "12345");

        let change = changeset.get_change("state.cash").unwrap();
        assert_eq!(change.as_u64(), Some(12345));
    }

    #[test]
    fn test_changeset_remove() {
        let mut changeset = ChangeSet::new();
        changeset.add(
            "state.cash".to_string(),
            serde_yaml::Value::Number(999.into()),
        );

        let removed = changeset.remove("state.cash");
        assert!(removed.is_some());
        assert!(changeset.is_empty());
    }

    #[test]
    fn test_changeset_clear() {
        let mut changeset = ChangeSet::new();
        changeset.add(
            "state.cash".to_string(),
            serde_yaml::Value::Number(999.into()),
        );
        changeset.add(
            "state.eridium".to_string(),
            serde_yaml::Value::Number(123.into()),
        );

        changeset.clear();
        assert!(changeset.is_empty());
    }

    #[test]
    fn test_changeset_apply() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let mut changeset = ChangeSet::new();

        changeset.add(
            "state.currencies.cash".to_string(),
            serde_yaml::Value::Number(5555.into()),
        );
        changeset.add(
            "state.currencies.eridium".to_string(),
            serde_yaml::Value::Number(6666.into()),
        );
        changeset.add(
            "state.char_name".to_string(),
            serde_yaml::Value::String("Modified".into()),
        );

        changeset.apply(&mut save).unwrap();

        assert_eq!(save.get_cash(), Some(5555));
        assert_eq!(save.get_eridium(), Some(6666));
        assert_eq!(save.get_character_name(), Some("Modified"));
    }

    #[test]
    fn test_changeset_convenience_methods() {
        let mut changeset = ChangeSet::new();

        changeset.set_character_name("TestName");
        changeset.set_cash(11111);
        changeset.set_eridium(22222);
        changeset.set_character_xp(33333);
        changeset.set_specialization_xp(44444);

        assert_eq!(changeset.len(), 5);
        assert!(changeset.has_change("state.char_name"));
        assert!(changeset.has_change("state.currencies.cash"));
        assert!(changeset.has_change("state.currencies.eridium"));
        assert!(changeset.has_change("state.experience[0].points"));
        assert!(changeset.has_change("state.experience[1].points"));
    }

    #[test]
    fn test_changeset_iter() {
        let mut changeset = ChangeSet::new();
        changeset.set_cash(1111);
        changeset.set_eridium(2222);

        let count = changeset.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_changeset_apply_multiple() {
        let mut save = SaveFile::from_yaml(test_save_yaml().as_bytes()).unwrap();
        let mut changeset = ChangeSet::new();

        changeset.set_cash(99999);
        changeset.set_eridium(88888);
        changeset.set_character_name("BatchTest");
        changeset.set_character_xp(77777);
        changeset.set_specialization_xp(66666);

        changeset.apply(&mut save).unwrap();

        assert_eq!(save.get_cash(), Some(99999));
        assert_eq!(save.get_eridium(), Some(88888));
        assert_eq!(save.get_character_name(), Some("BatchTest"));
        assert_eq!(save.get_character_level(), Some((10, 77777)));
        assert_eq!(save.get_specialization_level(), Some((5, 66666)));
    }

    // ─────────────────────────────────────────────────────────────────
    // StateFlags Tests
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_state_flags_backpack() {
        let flags = StateFlags::backpack();
        assert_eq!(flags.0, 513); // 1 (valid) + 512 (in_backpack)
        assert!(flags.is_in_backpack());
        assert!(!flags.is_equipped());
    }

    #[test]
    fn test_state_flags_equipped() {
        let flags = StateFlags::equipped();
        assert_eq!(flags.0, 1); // just valid
        assert!(!flags.is_in_backpack());
        assert!(flags.is_equipped());
    }

    #[test]
    fn test_state_flags_bank() {
        let flags = StateFlags::bank();
        assert_eq!(flags.0, 1); // just valid
    }

    #[test]
    fn test_state_flags_with_favorite() {
        let flags = StateFlags::backpack().with_favorite();
        assert_eq!(flags.0, 515); // 513 + 2
        assert!(flags.is_favorite());
        assert!(flags.is_in_backpack());
    }

    #[test]
    fn test_state_flags_with_junk() {
        let flags = StateFlags::backpack().with_junk();
        assert_eq!(flags.0, 517); // 513 + 4
        assert!(flags.is_junk());
    }

    #[test]
    fn test_state_flags_labels() {
        // Labels are mutually exclusive - only the last one set should be active
        let flags = StateFlags::backpack()
            .with_label2()
            .with_label3()
            .with_label4(); // Only label4 remains
        assert!(!flags.has_label2());
        assert!(!flags.has_label3());
        assert!(flags.has_label4());
        assert_eq!(flags.0, 513 + 128); // backpack + label4

        // Verify each label clears the others
        let fav = StateFlags::backpack().with_favorite();
        assert_eq!(fav.0, 515); // 513 + 2

        let junk = fav.with_junk(); // Changes from favorite to junk
        assert!(!junk.is_favorite());
        assert!(junk.is_junk());
        assert_eq!(junk.0, 517); // 513 + 4
    }

    #[test]
    fn test_state_flags_mutation() {
        let mut flags = StateFlags::backpack();
        assert!(!flags.is_favorite());

        flags.set_favorite(true);
        assert!(flags.is_favorite());
        assert_eq!(flags.0, 515); // 513 + 2 (favorite)

        flags.set_favorite(false);
        assert!(!flags.is_favorite());
        assert_eq!(flags.0, 513);
    }

    #[test]
    fn test_state_flags_to_equipped() {
        let backpack = StateFlags::backpack().with_favorite();
        let equipped = backpack.to_equipped();
        assert!(!equipped.is_in_backpack());
        assert!(equipped.is_favorite()); // preserves other flags
        assert_eq!(equipped.0, 3); // 1 + 2 (valid + favorite)
    }

    #[test]
    fn test_state_flags_to_backpack() {
        let equipped = StateFlags::equipped().with_junk();
        let backpack = equipped.to_backpack();
        assert!(backpack.is_in_backpack());
        assert!(backpack.is_junk()); // preserves other flags
        assert_eq!(backpack.0, 517); // 513 + 4 (backpack + junk)
    }

    #[test]
    fn test_state_flags_from_raw() {
        let flags = StateFlags::from_raw(515); // 513 + 2 = backpack + favorite
        assert!(flags.is_in_backpack());
        assert!(flags.is_favorite());
    }

    #[test]
    fn test_state_flags_conversions() {
        let flags = StateFlags::backpack();
        let raw: u32 = flags.into();
        assert_eq!(raw, 513);

        let restored: StateFlags = 515.into(); // 513 + 2 = backpack + favorite
        assert!(restored.is_favorite());
    }

    // ─────────────────────────────────────────────────────────────────
    // Inventory ChangeSet Tests
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_changeset_add_backpack_item() {
        let mut changeset = ChangeSet::new();
        changeset.add_backpack_item(5, "@TestSerial", StateFlags::backpack().with_favorite());

        assert!(changeset.has_change("state.inventory.items.backpack.slot_5.serial"));
        assert!(changeset.has_change("state.inventory.items.backpack.slot_5.flags"));
        assert!(changeset.has_change("state.inventory.items.backpack.slot_5.state_flags"));

        let state_flags = changeset
            .get_change("state.inventory.items.backpack.slot_5.state_flags")
            .unwrap();
        assert_eq!(state_flags.as_i64(), Some(515)); // 513 + 2 (backpack + favorite)
    }

    #[test]
    fn test_changeset_set_backpack_flags() {
        let mut changeset = ChangeSet::new();
        changeset.set_backpack_flags(3, StateFlags::backpack().with_junk());

        let change = changeset
            .get_change("state.inventory.items.backpack.slot_3.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(517)); // 513 + 4 (backpack + junk)
    }

    #[test]
    fn test_changeset_set_favorite() {
        let mut changeset = ChangeSet::new();
        changeset.set_favorite(0, true);

        let change = changeset
            .get_change("state.inventory.items.backpack.slot_0.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(515)); // 513 + 2 (backpack + favorite)
    }

    #[test]
    fn test_changeset_set_junk() {
        let mut changeset = ChangeSet::new();
        changeset.set_junk(1, true);

        let change = changeset
            .get_change("state.inventory.items.backpack.slot_1.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(517)); // 513 + 4 (backpack + junk)
    }

    #[test]
    fn test_changeset_add_bank_item() {
        let mut changeset = ChangeSet::new();
        changeset.add_bank_item(10, "@BankSerial", StateFlags::bank());

        assert!(changeset.has_change("domains.local.shared.inventory.items.bank.slot_10.serial"));
        assert!(
            changeset.has_change("domains.local.shared.inventory.items.bank.slot_10.state_flags")
        );
        // Bank items don't have a flags field
        assert!(!changeset.has_change("domains.local.shared.inventory.items.bank.slot_10.flags"));
    }

    #[test]
    fn test_changeset_equip_item() {
        let mut changeset = ChangeSet::new();
        changeset.equip_item(0, "@WeaponSerial");

        assert!(changeset.has_change("state.inventory.equipped_inventory.equipped.slot_0"));
    }

    #[test]
    fn test_changeset_unequip_slot() {
        let mut changeset = ChangeSet::new();
        changeset.unequip_slot(4);

        assert!(changeset.has_change("state.inventory.equipped_inventory.equipped.slot_4"));
    }

    // ─────────────────────────────────────────────────────────────────
    // Additional StateFlags Coverage Tests
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_state_flags_set_junk_mutation() {
        let mut flags = StateFlags::backpack();
        flags.set_junk(true);
        assert!(flags.is_junk());
        assert!(!flags.is_favorite()); // Labels are mutually exclusive

        flags.set_junk(false);
        assert!(!flags.is_junk());
    }

    #[test]
    fn test_state_flags_set_label1() {
        let mut flags = StateFlags::backpack();
        flags.set_label1(true);
        assert!(flags.has_label1());
        assert!(!flags.is_favorite());

        flags.set_label1(false);
        assert!(!flags.has_label1());
    }

    #[test]
    fn test_state_flags_set_label2() {
        let mut flags = StateFlags::backpack();
        flags.set_label2(true);
        assert!(flags.has_label2());

        flags.set_label2(false);
        assert!(!flags.has_label2());
    }

    #[test]
    fn test_state_flags_set_label3() {
        let mut flags = StateFlags::backpack();
        flags.set_label3(true);
        assert!(flags.has_label3());

        flags.set_label3(false);
        assert!(!flags.has_label3());
    }

    #[test]
    fn test_state_flags_set_label4() {
        let mut flags = StateFlags::backpack();
        flags.set_label4(true);
        assert!(flags.has_label4());

        flags.set_label4(false);
        assert!(!flags.has_label4());
    }

    #[test]
    fn test_state_flags_clear_labels() {
        let mut flags = StateFlags::backpack().with_favorite();
        assert!(flags.is_favorite());

        flags.clear_labels();
        assert!(!flags.is_favorite());
        assert!(!flags.is_junk());
        assert!(!flags.has_label1());
        assert!(flags.is_in_backpack()); // Non-label flags preserved
    }

    #[test]
    fn test_state_flags_with_label1() {
        let flags = StateFlags::backpack().with_label1();
        assert!(flags.has_label1());
        assert!(!flags.is_favorite());
        assert!(!flags.is_junk());
    }

    #[test]
    fn test_state_flags_with_no_label() {
        let flags = StateFlags::backpack().with_favorite().with_no_label();
        assert!(!flags.is_favorite());
        assert!(!flags.is_junk());
        assert!(!flags.has_label1());
        assert!(flags.is_in_backpack());
    }

    #[test]
    fn test_state_flags_to_raw() {
        let flags = StateFlags::backpack();
        assert_eq!(flags.to_raw(), 513);
    }

    // ─────────────────────────────────────────────────────────────────
    // Additional ChangeSet Coverage Tests
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_changeset_set_label1() {
        let mut changeset = ChangeSet::new();
        changeset.set_label1(2, true);

        let change = changeset
            .get_change("state.inventory.items.backpack.slot_2.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(529)); // 513 + 16 (backpack + label1)
    }

    #[test]
    fn test_changeset_set_label2() {
        let mut changeset = ChangeSet::new();
        changeset.set_label2(3, true);

        let change = changeset
            .get_change("state.inventory.items.backpack.slot_3.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(545)); // 513 + 32 (backpack + label2)
    }

    #[test]
    fn test_changeset_set_label3() {
        let mut changeset = ChangeSet::new();
        changeset.set_label3(4, true);

        let change = changeset
            .get_change("state.inventory.items.backpack.slot_4.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(577)); // 513 + 64 (backpack + label3)
    }

    #[test]
    fn test_changeset_set_label4() {
        let mut changeset = ChangeSet::new();
        changeset.set_label4(5, true);

        let change = changeset
            .get_change("state.inventory.items.backpack.slot_5.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(641)); // 513 + 128 (backpack + label4)
    }

    #[test]
    fn test_changeset_set_bank_flags() {
        let mut changeset = ChangeSet::new();
        changeset.set_bank_flags(42, StateFlags::bank().with_favorite());

        let change = changeset
            .get_change("domains.local.shared.inventory.items.bank.slot_42.state_flags")
            .unwrap();
        assert_eq!(change.as_i64(), Some(3)); // 1 + 2 (valid + favorite)
    }

    #[test]
    fn test_changeset_add_raw() {
        let mut changeset = ChangeSet::new();
        let result =
            changeset.add_raw("some.path".to_string(), "key: value\nnested:\n  field: 123");
        assert!(result.is_ok());
        assert!(changeset.has_change("some.path"));
    }

    #[test]
    fn test_changeset_add_raw_invalid_yaml() {
        let mut changeset = ChangeSet::new();
        let result = changeset.add_raw("some.path".to_string(), "invalid: yaml: :::");
        assert!(result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────
    // Additional parse_value Coverage Tests
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_value_float() {
        let val = SaveFile::parse_value("3.14159");
        assert!(val.as_f64().is_some());
        assert!((val.as_f64().unwrap() - 3.14159).abs() < 0.0001);
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

    // ─────────────────────────────────────────────────────────────────
    // SaveFile Debug and Error Coverage Tests
    // ─────────────────────────────────────────────────────────────────

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
