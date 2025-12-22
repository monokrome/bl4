// Abstracts Tauri IPC vs HTTP fetch for desktop/server modes

const isTauri = '__TAURI__' in window;

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
}

interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

async function httpPost<T>(endpoint: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  });
  const json: ApiResponse<T> = await res.json();
  if (!json.success) throw new Error(json.error || 'Unknown error');
  return json.data as T;
}

async function httpGet<T>(endpoint: string): Promise<T> {
  const res = await fetch(`/api${endpoint}`);
  const json: ApiResponse<T> = await res.json();
  if (!json.success) throw new Error(json.error || 'Unknown error');
  return json.data as T;
}

export async function openSave(path: string, steamId: string): Promise<SaveInfo> {
  if (isTauri) {
    return tauriInvoke<SaveInfo>('open_save', { path, steamId });
  }
  return httpPost<SaveInfo>('/save/open', { path, steam_id: steamId });
}

export async function saveChanges(): Promise<void> {
  if (isTauri) {
    return tauriInvoke<void>('save_changes');
  }
  return httpPost<void>('/save');
}

export async function getSaveInfo(): Promise<SaveInfo | null> {
  if (isTauri) {
    return tauriInvoke<SaveInfo | null>('get_save_info');
  }
  return httpGet<SaveInfo | null>('/save/info');
}

export async function getCharacter(): Promise<CharacterInfo> {
  if (isTauri) {
    return tauriInvoke<CharacterInfo>('get_character');
  }
  return httpGet<CharacterInfo>('/character');
}

export async function setCharacter(request: SetCharacterRequest): Promise<void> {
  if (isTauri) {
    return tauriInvoke<void>('set_character', { request });
  }
  return httpPost<void>('/character', request);
}

export async function getInventory(): Promise<InventoryItem[]> {
  if (isTauri) {
    return tauriInvoke<InventoryItem[]>('get_inventory');
  }
  return httpGet<InventoryItem[]>('/inventory');
}

export async function connectDb(path: string): Promise<void> {
  if (isTauri) {
    return tauriInvoke<void>('connect_db', { path });
  }
  return httpPost<void>('/db/connect', { path });
}

export async function syncToBank(serials: string[]): Promise<number> {
  if (isTauri) {
    return tauriInvoke<number>('sync_to_bank', { serials });
  }
  return httpPost<number>('/bank/sync', { serials });
}

// Tauri file dialog (desktop only)
export async function selectFile(filters?: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (!isTauri) return null;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const result = await open({
    multiple: false,
    filters,
  });
  return result as string | null;
}
