import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import type {
  SaveInfo,
  CharacterInfo,
  SetCharacterRequest,
  InventoryItem,
  ItemDetail,
  BankInfo,
} from '../types'

export function openFileDialog() {
  return open({
    multiple: false,
    filters: [{ name: 'Save', extensions: ['sav'] }],
  })
}

export function saveFileDialog() {
  return save({
    filters: [{ name: 'Save', extensions: ['sav'] }],
  })
}

export function openSave(path: string, steamId: string) {
  return invoke<SaveInfo>('open_save', { path, steamId })
}

export function getSaveInfo() {
  return invoke<SaveInfo | null>('get_save_info')
}

export function getCharacter() {
  return invoke<CharacterInfo>('get_character')
}

export function setCharacter(request: SetCharacterRequest) {
  return invoke<void>('set_character', { request })
}

export function getInventory() {
  return invoke<InventoryItem[]>('get_inventory')
}

export function getItemDetail(serial: string) {
  return invoke<ItemDetail>('get_item_detail', { serial })
}

export function getBank() {
  return invoke<BankInfo>('get_bank')
}

export function saveChanges() {
  return invoke<void>('save_changes')
}
