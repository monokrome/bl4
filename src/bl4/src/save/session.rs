//! Save session: multi-file save context for cross-file validation.
//!
//! SaveSession holds character and profile saves together, enabling
//! operations that require cross-file context (e.g., entitlement checks).

// TODO: Implementation will go here after tests pass on the current API.

#[cfg(test)]
mod tests {
    use crate::save::{ChangeSet, SaveFile, StateFlags};

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

    // --- Entitlement detection ---

    #[test]
    fn entitlements_detected_from_profile() {
        let profile = SaveFile::from_yaml(profile_yaml_with_entitlements().as_bytes()).unwrap();
        let ent = profile.entitlements();
        assert!(ent.preorder, "should detect pre-order");
        assert!(ent.premium_edition, "should detect premium edition");
        assert!(ent.golden_power, "should detect golden power");
    }

    #[test]
    fn entitlements_absent_when_not_owned() {
        let profile = SaveFile::from_yaml(profile_yaml_no_entitlements().as_bytes()).unwrap();
        let ent = profile.entitlements();
        assert!(!ent.preorder);
        assert!(!ent.premium_edition);
        assert!(!ent.golden_power);
    }

    #[test]
    fn entitlements_on_character_save_returns_defaults() {
        let character = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let ent = character.entitlements();
        assert!(!ent.preorder);
        assert!(!ent.premium_edition);
        assert!(!ent.golden_power);
    }

    // --- Character save operations (must work without profile) ---

    #[test]
    fn character_cash_roundtrip() {
        let mut save = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        assert_eq!(save.get_cash(), Some(1000));

        let mut cs = ChangeSet::new();
        cs.set_cash(99999);
        cs.apply(&mut save).unwrap();
        assert_eq!(save.get_cash(), Some(99999));
    }

    #[test]
    fn character_xp_roundtrip() {
        let mut save = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        assert_eq!(save.get_character_level(), Some((50, 3430207)));

        let mut cs = ChangeSet::new();
        cs.set_character_xp(5714893);
        cs.apply(&mut save).unwrap();
        assert_eq!(save.get_character_level(), Some((50, 5714893)));
    }

    #[test]
    fn character_add_backpack_item() {
        let mut save = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let mut cs = ChangeSet::new();
        cs.add_backpack_item(0, "@TestNewItem", StateFlags::backpack());
        cs.apply(&mut save).unwrap();

        let yaml = save.to_yaml().unwrap();
        let yaml_str = String::from_utf8(yaml).unwrap();
        assert!(yaml_str.contains("@TestNewItem"));
    }

    #[test]
    fn character_equip_and_unequip() {
        let mut save = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let mut cs = ChangeSet::new();
        cs.equip_item(0, "@TestWeapon");
        cs.apply(&mut save).unwrap();

        let yaml = save.to_yaml().unwrap();
        let yaml_str = String::from_utf8(yaml).unwrap();
        assert!(yaml_str.contains("@TestWeapon"));

        let mut cs2 = ChangeSet::new();
        cs2.unequip_slot(0);
        cs2.apply(&mut save).unwrap();
    }

    // --- Profile save operations ---

    #[test]
    fn profile_add_bank_item() {
        let mut profile =
            SaveFile::from_yaml(profile_yaml_with_entitlements().as_bytes()).unwrap();
        let mut cs = ChangeSet::new();
        cs.add_bank_item(0, "@TestBankItem", StateFlags::bank());
        cs.apply(&mut profile).unwrap();

        let yaml = profile.to_yaml().unwrap();
        let yaml_str = String::from_utf8(yaml).unwrap();
        assert!(yaml_str.contains("@TestBankItem"));
    }

    // --- Serial collection ---

    #[test]
    fn collect_serials_from_character() {
        let save = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let serials = save.collect_serial_paths();
        // backpack slot_0, slot_1, equipped slot_0 = 3 serials
        assert_eq!(serials.len(), 3);
        assert!(serials.iter().all(|(_, s)| s.starts_with('@')));
    }

    #[test]
    fn collect_serials_from_profile() {
        let profile =
            SaveFile::from_yaml(profile_yaml_with_entitlements().as_bytes()).unwrap();
        let serials = profile.collect_serial_paths();
        assert_eq!(serials.len(), 1);
    }

    // --- Item level operations ---

    #[test]
    fn set_all_item_levels_updates_count() {
        let save = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let mut cs = ChangeSet::new();
        let count = cs.set_all_item_levels(&save, 60);
        // Real serials that decode successfully get updated
        assert!(count > 0, "should update at least some items");
    }

    // --- Multiple changesets don't interfere ---

    #[test]
    fn independent_changesets_on_different_files() {
        let mut character = SaveFile::from_yaml(character_yaml().as_bytes()).unwrap();
        let mut profile =
            SaveFile::from_yaml(profile_yaml_with_entitlements().as_bytes()).unwrap();

        let mut char_cs = ChangeSet::new();
        char_cs.set_cash(50000);

        let mut prof_cs = ChangeSet::new();
        prof_cs.add_bank_item(0, "@BankItem", StateFlags::bank());

        char_cs.apply(&mut character).unwrap();
        prof_cs.apply(&mut profile).unwrap();

        assert_eq!(character.get_cash(), Some(50000));
        let prof_yaml = String::from_utf8(profile.to_yaml().unwrap()).unwrap();
        assert!(prof_yaml.contains("@BankItem"));
    }
}
