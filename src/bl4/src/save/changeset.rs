//! Batch change tracking for save file modifications.

use std::collections::HashMap;

use super::{parse_value, SaveError, SaveFile, StateFlags};

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

    /// Set item level on all serials in a save file.
    ///
    /// Decodes each serial, re-encodes at the target level, and adds
    /// the updated serial to the changeset. Returns the number of
    /// serials successfully updated.
    pub fn set_all_item_levels(&mut self, save: &super::SaveFile, level: u8) -> u32 {
        let mut count = 0u32;
        for (path, serial) in save.collect_serial_paths() {
            let Ok(item) = crate::serial::ItemSerial::decode(&serial) else {
                continue;
            };
            let Some(modified) = item.with_level(level) else {
                continue;
            };
            let new_serial = modified.encode_from_tokens();
            self.add(path, serde_yaml::Value::String(new_serial));
            count += 1;
        }
        count
    }

    /// Clear an equipped slot (unequip item).
    pub fn unequip_slot(&mut self, slot: u8) {
        let _ = self.add_raw(
            format!("state.inventory.equipped_inventory.equipped.slot_{}", slot),
            "[]",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
