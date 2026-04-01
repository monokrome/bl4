//! Save session: multi-file save context for cross-file validation.
//!
//! SaveSession holds character and profile saves together, enabling
//! operations that require cross-file context (e.g., entitlement checks).
//! Each save file is paired with its own ChangeSet for tracking mutations.

use super::entitlements::Entitlements;
use super::{ChangeSet, SaveError, SaveFile, StateFlags};

/// A save file paired with its pending changes.
pub struct SaveState {
    pub name: String,
    pub file: SaveFile,
    pub changes: ChangeSet,
}

impl SaveState {
    pub fn new(name: impl Into<String>, file: SaveFile) -> Self {
        Self {
            name: name.into(),
            file,
            changes: ChangeSet::new(),
        }
    }

    /// Apply pending changes to the save file.
    pub fn apply(&mut self) -> Result<(), SaveError> {
        self.changes.apply(&mut self.file)?;
        self.changes.clear();
        Ok(())
    }

    /// Whether there are unapplied changes.
    pub fn is_dirty(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Serialize the save file to YAML bytes.
    pub fn to_yaml(&self) -> Result<Vec<u8>, SaveError> {
        self.file.to_yaml()
    }
}

/// Multi-file save context.
///
/// Holds character saves and an optional profile save, routing
/// operations to the correct SaveState and validating entitlements
/// when required.
pub struct SaveSession {
    pub characters: Vec<SaveState>,
    pub profile: Option<SaveState>,
}

impl SaveSession {
    pub fn new() -> Self {
        Self {
            characters: Vec::new(),
            profile: None,
        }
    }

    /// Create a session with a single character save.
    pub fn with_character(name: impl Into<String>, file: SaveFile) -> Self {
        Self {
            characters: vec![SaveState::new(name, file)],
            profile: None,
        }
    }

    /// Add a character save to the session.
    pub fn add_character(&mut self, name: impl Into<String>, file: SaveFile) {
        self.characters.push(SaveState::new(name, file));
    }

    /// Set the profile save.
    pub fn set_profile(&mut self, file: SaveFile) {
        self.profile = Some(SaveState::new("profile", file));
    }

    /// Get a character SaveState by name.
    pub fn character(&self, name: &str) -> Option<&SaveState> {
        self.characters.iter().find(|c| c.name == name)
    }

    /// Get a mutable character SaveState by name.
    pub fn character_mut(&mut self, name: &str) -> Option<&mut SaveState> {
        self.characters.iter_mut().find(|c| c.name == name)
    }

    /// Detect entitlements from the profile save.
    ///
    /// Returns default (no entitlements) if no profile is loaded.
    pub fn entitlements(&self) -> Entitlements {
        self.profile
            .as_ref()
            .map(|p| p.file.entitlements())
            .unwrap_or_default()
    }

    // ─────────────────────────────────────────────────────────────────
    // Character Operations
    // ─────────────────────────────────────────────────────────────────

    /// Set cash on a character save.
    pub fn set_cash(&mut self, character: &str, amount: u64) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.set_cash(amount);
        Ok(())
    }

    /// Set eridium on a character save.
    pub fn set_eridium(&mut self, character: &str, amount: u64) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.set_eridium(amount);
        Ok(())
    }

    /// Set character name.
    pub fn set_character_name(&mut self, character: &str, name: &str) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.set_character_name(name);
        Ok(())
    }

    /// Set character XP.
    pub fn set_character_xp(&mut self, character: &str, xp: u64) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.set_character_xp(xp);
        Ok(())
    }

    /// Set specialization XP.
    pub fn set_specialization_xp(&mut self, character: &str, xp: u64) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.set_specialization_xp(xp);
        Ok(())
    }

    /// Add an item to a character's backpack.
    pub fn add_backpack_item(
        &mut self,
        character: &str,
        slot: u8,
        serial: &str,
        flags: StateFlags,
    ) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.add_backpack_item(slot, serial, flags);
        Ok(())
    }

    /// Equip an item to a character slot.
    pub fn equip_item(&mut self, character: &str, slot: u8, serial: &str) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.equip_item(slot, serial);
        Ok(())
    }

    /// Unequip a character slot.
    pub fn unequip_slot(&mut self, character: &str, slot: u8) -> Result<(), SaveError> {
        let state = self.require_character(character)?;
        state.changes.unequip_slot(slot);
        Ok(())
    }

    /// Set all item levels on a character save.
    pub fn set_item_level(&mut self, character: &str, level: u8) -> Result<u32, SaveError> {
        let state = self.require_character(character)?;
        let count = state.changes.set_all_item_levels(&state.file, level);
        Ok(count)
    }

    // ─────────────────────────────────────────────────────────────────
    // Profile Operations
    // ─────────────────────────────────────────────────────────────────

    /// Add an item to the bank (profile.sav).
    pub fn add_bank_item(
        &mut self,
        slot: u16,
        serial: &str,
        flags: StateFlags,
    ) -> Result<(), SaveError> {
        let state = self.require_profile()?;
        state.changes.add_bank_item(slot, serial, flags);
        Ok(())
    }

    /// Set all item levels on the profile save (bank items).
    pub fn set_bank_item_level(&mut self, level: u8) -> Result<u32, SaveError> {
        let state = self.require_profile()?;
        let count = state.changes.set_all_item_levels(&state.file, level);
        Ok(count)
    }

    // ─────────────────────────────────────────────────────────────────
    // Batch Operations
    // ─────────────────────────────────────────────────────────────────

    /// Apply all pending changes across all saves.
    pub fn apply_all(&mut self) -> Result<(), SaveError> {
        for state in &mut self.characters {
            state.apply()?;
        }
        if let Some(ref mut profile) = self.profile {
            profile.apply()?;
        }
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Internal Helpers
    // ─────────────────────────────────────────────────────────────────

    fn require_character(&mut self, name: &str) -> Result<&mut SaveState, SaveError> {
        self.characters
            .iter_mut()
            .find(|c| c.name == name)
            .ok_or_else(|| SaveError::KeyNotFound(format!("character '{}'", name)))
    }

    fn require_profile(&mut self) -> Result<&mut SaveState, SaveError> {
        self.profile
            .as_mut()
            .ok_or_else(|| SaveError::KeyNotFound("profile save not loaded".to_string()))
    }
}

impl Default for SaveSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character_yaml() -> &'static str {
        r#"
state:
  char_name: TestChar
  class: Char_DarkSiren
  player_difficulty: Normal
  currencies:
    cash: 1000
    eridium: 50
  experience:
    - type: Character
      level: 50
      points: 3430207
    - type: Specialization
      level: 5
      points: 2500
  inventory:
    items:
      backpack:
        slot_0:
          serial: "@Ugr$ZCm/&tH!t{KgK/Shxu>k"
          flags: 1
          state_flags: 513
        slot_1:
          serial: "@UgbV{rFme!KAVknuRG/{Js74(sEGiwUA8H+{Q)^IpP@_<xP_s}e5d"
          flags: 1
          state_flags: 513
    equipped_inventory:
      equipped:
        slot_0:
        - serial: "@Ugr$ZCm/&tH!t{KgK/Shxu>k"
          flags: 1
          state_flags: 1
save_game_header:
  guid: TEST-CHAR-001
"#
    }

    fn profile_yaml_with_entitlements() -> &'static str {
        r#"
domains:
  local:
    characters_selected: C_1
    shared:
      currencies:
        vaultcard01_tokens: 100
      inventory:
        items:
          bank:
            slot_0:
              serial: "@Ugr$ZCm/&tH!t{KgK/Shxu>k"
              state_flags: 1
    unlockables:
      unlockable_darksiren:
        entries:
        - Unlockable_DarkSiren.Skin24_PreOrder
        - Unlockable_DarkSiren.Body02_Premium
        - Unlockable_DarkSiren.Head16_Premium
        - Unlockable_DarkSiren.Skin44_Premium
      unlockable_weapons:
        entries:
        - Unlockable_Weapons.Mat36_PreOrder
        - Unlockable_Weapons.Mat27_GoldenPower
      unlockable_vehicles:
        entries:
        - Unlockable_Vehicles.Mat27_GoldenPower
save_game_header:
  guid: TEST-PROFILE-001
"#
    }

    fn profile_yaml_no_entitlements() -> &'static str {
        r#"
domains:
  local:
    characters_selected: C_1
    shared:
      currencies:
        vaultcard01_tokens: 0
      inventory:
        items:
          bank: {}
    unlockables:
      unlockable_darksiren:
        entries:
        - Unlockable_DarkSiren.Head01_Prison
        - Unlockable_DarkSiren.Skin01_Prison
save_game_header:
  guid: TEST-PROFILE-002
"#
    }

    fn make_session() -> SaveSession {
        let char_file = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let profile_file =
            SaveFile::from_yaml(profile_yaml_with_entitlements().as_bytes()).unwrap();
        let mut session = SaveSession::with_character("1", char_file);
        session.set_profile(profile_file);
        session
    }

    fn make_session_no_profile() -> SaveSession {
        let char_file = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        SaveSession::with_character("1", char_file)
    }

    // --- Entitlement detection ---

    #[test]
    fn entitlements_detected_from_profile() {
        let session = make_session();
        let ent = session.entitlements();
        assert!(ent.preorder);
        assert!(ent.premium_edition);
        assert!(ent.golden_power);
    }

    #[test]
    fn entitlements_absent_when_not_owned() {
        let profile = SaveFile::from_yaml(profile_yaml_no_entitlements().as_bytes()).unwrap();
        let mut session = make_session_no_profile();
        session.set_profile(profile);
        let ent = session.entitlements();
        assert!(!ent.preorder);
        assert!(!ent.premium_edition);
        assert!(!ent.golden_power);
    }

    #[test]
    fn entitlements_default_without_profile() {
        let session = make_session_no_profile();
        let ent = session.entitlements();
        assert!(!ent.preorder);
        assert!(!ent.premium_edition);
        assert!(!ent.golden_power);
    }

    // --- Character operations ---

    #[test]
    fn set_cash_on_character() {
        let mut session = make_session();
        session.set_cash("1", 99999).unwrap();
        assert!(session.character("1").unwrap().is_dirty());
        session.apply_all().unwrap();
        assert_eq!(session.character("1").unwrap().file.get_cash(), Some(99999));
    }

    #[test]
    fn set_xp_on_character() {
        let mut session = make_session();
        session.set_character_xp("1", 5714893).unwrap();
        session.apply_all().unwrap();
        assert_eq!(
            session.character("1").unwrap().file.get_character_level(),
            Some((50, 5714893))
        );
    }

    #[test]
    fn add_backpack_item_on_character() {
        let mut session = make_session();
        session
            .add_backpack_item("1", 0, "@TestNewItem", StateFlags::backpack())
            .unwrap();
        session.apply_all().unwrap();
        let yaml = String::from_utf8(session.character("1").unwrap().to_yaml().unwrap()).unwrap();
        assert!(yaml.contains("@TestNewItem"));
    }

    #[test]
    fn equip_and_unequip() {
        let mut session = make_session();
        session.equip_item("1", 0, "@TestWeapon").unwrap();
        session.apply_all().unwrap();
        let yaml = String::from_utf8(session.character("1").unwrap().to_yaml().unwrap()).unwrap();
        assert!(yaml.contains("@TestWeapon"));

        session.unequip_slot("1", 0).unwrap();
        session.apply_all().unwrap();
    }

    #[test]
    fn set_item_level_on_character() {
        let mut session = make_session();
        let count = session.set_item_level("1", 60).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn character_not_found_errors() {
        let mut session = make_session();
        assert!(session.set_cash("nonexistent", 100).is_err());
    }

    // --- Profile operations ---

    #[test]
    fn add_bank_item_on_profile() {
        let mut session = make_session();
        session
            .add_bank_item(0, "@TestBankItem", StateFlags::bank())
            .unwrap();
        session.apply_all().unwrap();
        let yaml = String::from_utf8(session.profile.as_ref().unwrap().to_yaml().unwrap()).unwrap();
        assert!(yaml.contains("@TestBankItem"));
    }

    #[test]
    fn profile_required_for_bank_ops() {
        let mut session = make_session_no_profile();
        assert!(session
            .add_bank_item(0, "@Item", StateFlags::bank())
            .is_err());
    }

    // --- Multi-character ---

    #[test]
    fn multiple_characters() {
        let char1 = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let char2 = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let mut session = SaveSession::new();
        session.add_character("1", char1);
        session.add_character("2", char2);

        session.set_cash("1", 11111).unwrap();
        session.set_cash("2", 22222).unwrap();
        session.apply_all().unwrap();

        assert_eq!(session.character("1").unwrap().file.get_cash(), Some(11111));
        assert_eq!(session.character("2").unwrap().file.get_cash(), Some(22222));
    }

    // --- Dirty tracking ---

    #[test]
    fn dirty_flag_tracks_changes() {
        let mut session = make_session();
        assert!(!session.character("1").unwrap().is_dirty());

        session.set_cash("1", 5000).unwrap();
        assert!(session.character("1").unwrap().is_dirty());

        session.apply_all().unwrap();
        assert!(!session.character("1").unwrap().is_dirty());
    }

    // --- Cross-file independence ---

    #[test]
    fn character_and_profile_changes_independent() {
        let mut session = make_session();
        session.set_cash("1", 50000).unwrap();
        session
            .add_bank_item(0, "@BankItem", StateFlags::bank())
            .unwrap();
        session.apply_all().unwrap();

        assert_eq!(session.character("1").unwrap().file.get_cash(), Some(50000));
        let prof_yaml =
            String::from_utf8(session.profile.as_ref().unwrap().to_yaml().unwrap()).unwrap();
        assert!(prof_yaml.contains("@BankItem"));
    }

    // --- Serial collection ---

    #[test]
    fn collect_serials_from_character() {
        let session = make_session();
        let serials = session.character("1").unwrap().file.collect_serial_paths();
        assert_eq!(serials.len(), 3);
        assert!(serials.iter().all(|(_, s)| s.starts_with('@')));
    }

    #[test]
    fn collect_serials_from_profile() {
        let session = make_session();
        let serials = session
            .profile
            .as_ref()
            .unwrap()
            .file
            .collect_serial_paths();
        assert_eq!(serials.len(), 1);
    }
}
