import { readFile, readDir } from '@tauri-apps/plugin-fs';
import { initBl4, decryptSav, SaveFile } from './bl4.js';
import { type SaveInfo, type EditorState } from './contexts.js';

function joinPath(dir: string, name: string): string {
  const sep = dir.includes('\\') ? '\\' : '/';
  return `${dir.replace(/[\\/]+$/, '')}${sep}${name}`;
}

export function extractSteamIdFromPath(path: string): string | null {
  const match = path.match(/(\d{17})/);
  return match ? match[1] : null;
}

async function tryDecrypt(bytes: Uint8Array, pathSid: string | null, inputSid: string): Promise<SaveFile> {
  if (pathSid) {
    try { return new SaveFile(decryptSav(bytes, pathSid)); } catch { /* fall through */ }
  }
  if (inputSid && inputSid !== pathSid) {
    try { return new SaveFile(decryptSav(bytes, inputSid)); } catch { /* fall through */ }
  }
  throw new Error('Decryption failed — check your Steam ID');
}

async function parseSave(path: string, filename: string, sid: string, classDisplayName: (r: string | null) => string): Promise<SaveInfo | null> {
  try {
    const bytes = await readFile(path);
    const save = await tryDecrypt(new Uint8Array(bytes), extractSteamIdFromPath(path), sid);

    const isProfile = filename.toLowerCase().includes('profile');
    const name = isProfile ? 'profile' : filename.replace('.sav', '');

    if (isProfile) {
      save.free();
      return { path, name, isProfile: true, characterName: null, characterClass: null, level: null, difficulty: null, hint: 'bank, unlockables' };
    }

    const charName = save.getCharacterName() ?? null;
    const charClass = save.getCharacterClass() ?? null;
    const levelData = save.getCharacterLevel();
    const level = levelData ? levelData[0] : null;
    const difficulty = save.getDifficulty() ?? null;
    const displayClass = classDisplayName(charClass);
    const parts = [level ? `Lv${level}` : null, displayClass, difficulty].filter(Boolean);

    save.free();
    return { path, name, isProfile: false, characterName: charName, characterClass: charClass, level, difficulty, hint: parts.join(' / ') };
  } catch (e) {
    console.error(`Failed to parse ${filename}:`, e);
    const isProfile = filename.toLowerCase().includes('profile');
    const name = isProfile ? 'profile' : filename.replace('.sav', '');
    return { path, name, isProfile, characterName: null, characterClass: null, level: null, difficulty: null, hint: 'failed to decrypt' };
  }
}

export async function loadDirectory(dir: string, sid: string, editor: EditorState, classDisplayName: (r: string | null) => string) {
  editor.loading = true;
  editor.error = null;

  try {
    await initBl4();
    const entries = await readDir(dir);
    const savFiles = entries
      .filter(e => e.name?.endsWith('.sav') && !e.name.endsWith('.sav.bak'))
      .sort((a, b) => (a.name ?? '').localeCompare(b.name ?? ''));

    const saves: SaveInfo[] = [];
    for (const entry of savFiles) {
      const path = joinPath(dir, entry.name!);
      const info = await parseSave(path, entry.name ?? '', sid, classDisplayName);
      if (info) saves.push(info);
    }

    saves.sort((a, b) => {
      if (a.isProfile && !b.isProfile) return -1;
      if (!a.isProfile && b.isProfile) return 1;
      return (b.level ?? 0) - (a.level ?? 0);
    });

    editor.saves = saves;
  } catch (e) {
    editor.error = String(e);
  } finally {
    editor.loading = false;
  }
}

export async function loadSingleFile(path: string, sid: string, editor: EditorState, classDisplayName: (r: string | null) => string) {
  editor.loading = true;
  editor.error = null;

  try {
    await initBl4();
    const name = path.split('/').pop() ?? path;
    const info = await parseSave(path, name, sid, classDisplayName);
    if (info) editor.saves = [info];
  } catch (e) {
    editor.error = String(e);
  } finally {
    editor.loading = false;
  }
}
