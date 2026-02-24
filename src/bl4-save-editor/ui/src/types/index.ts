// Shared TypeScript types for the BL4 Save Editor UI

export interface SaveInfo {
  path: string;
  is_profile: boolean;
  modified: boolean;
  character_name: string | null;
  character_class: string | null;
  difficulty: string | null;
}

export interface CharacterInfo {
  name: string | null;
  class: string | null;
  difficulty: string | null;
  level: number | null;
  xp: number | null;
  specialization_level: number | null;
  specialization_xp: number | null;
  cash: number | null;
  eridium: number | null;
}

export interface SetCharacterRequest {
  name?: string;
  cash?: number;
  eridium?: number;
  xp?: number;
  specialization_xp?: number;
}

export interface InventoryItem {
  slot: number;
  serial: string;
  state_flags: number;
  is_favorite: boolean;
  is_junk: boolean;
  is_equipped: boolean;
  // Decoded info
  name: string | null;
  level: number | null;
  manufacturer: string | null;
  weapon_type: string | null;
  rarity: string | null;
  elements: string | null;
  item_type: string | null;
  decode_success: boolean;
}

export interface ItemDetail {
  serial: string;
  item_type: string;
  item_type_name: string;
  manufacturer: string | null;
  weapon_type: string | null;
  level: number | null;
  rarity: string | null;
  elements: string | null;
  parts: PartDetail[];
  decode_success: boolean;
  level_editable: boolean;
  element_editable: boolean;
}

export interface PartDetail {
  index: number;
  category: string | null;
  name: string | null;
}

export interface BankInfo {
  items: InventoryItem[];
  count: number;
  max_capacity: number;
  sdu_warning: boolean;
}

