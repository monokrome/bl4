import { readFile, readDir } from '@tauri-apps/plugin-fs';
import { initBl4, decryptSav, encryptSav, SaveFile } from './bl4.js';
import { type SaveInfo, type SaveSession, type EditorState } from './contexts.js';
import { createChangeSet, disposeSession } from './editor-store.js';

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

interface ParseResult {
  info: SaveInfo;
  file: SaveFile | null;
}

async function parseSave(path: string, filename: string, sid: string, classDisplayName: (r: string | null) => string): Promise<ParseResult> {
  try {
    const bytes = await readFile(path);
    const file = await tryDecrypt(new Uint8Array(bytes), extractSteamIdFromPath(path), sid);

    const isProfile = filename.toLowerCase().includes('profile');
    const name = isProfile ? 'profile' : filename.replace('.sav', '');

    if (isProfile) {
      return {
        info: { path, name, isProfile: true, characterName: null, characterClass: null, level: null, difficulty: null, hint: 'bank, unlockables' },
        file,
      };
    }

    const charName = file.getCharacterName() ?? null;
    const charClass = file.getCharacterClass() ?? null;
    const levelData = file.getCharacterLevel();
    const level = levelData ? levelData[0] : null;
    const difficulty = file.getDifficulty() ?? null;
    const displayClass = classDisplayName(charClass);
    const parts = [level ? `Lv${level}` : null, displayClass, difficulty].filter(Boolean);

    return {
      info: { path, name, isProfile: false, characterName: charName, characterClass: charClass, level, difficulty, hint: parts.join(' / ') },
      file,
    };
  } catch (e) {
    console.error(`Failed to parse ${filename}:`, e);
    const isProfile = filename.toLowerCase().includes('profile');
    const name = isProfile ? 'profile' : filename.replace('.sav', '');
    return {
      info: { path, name, isProfile, characterName: null, characterClass: null, level: null, difficulty: null, hint: 'failed to decrypt' },
      file: null,
    };
  }
}

/// Build a SaveSession from a successfully parsed save.
function buildSession(info: SaveInfo, file: SaveFile): SaveSession {
  return {
    info,
    file,
    changes: createChangeSet(),
    dirty: {},
  };
}

/// Dispose all existing sessions before loading new ones.
function clearSessions(editor: EditorState): void {
  for (const session of Object.values(editor.sessions)) {
    disposeSession(session);
  }
  for (const key of Object.keys(editor.sessions)) {
    delete editor.sessions[key];
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

    clearSessions(editor);
    const saves: SaveInfo[] = [];

    for (const entry of savFiles) {
      const path = joinPath(dir, entry.name!);
      const result = await parseSave(path, entry.name ?? '', sid, classDisplayName);
      saves.push(result.info);
      if (result.file) {
        editor.sessions[result.info.name] = buildSession(result.info, result.file);
      }
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
    const result = await parseSave(path, name, sid, classDisplayName);
    clearSessions(editor);
    editor.saves = [result.info];
    if (result.file) {
      editor.sessions[result.info.name] = buildSession(result.info, result.file);
    }
  } catch (e) {
    editor.error = String(e);
  } finally {
    editor.loading = false;
  }
}

/// Apply all pending changes in a session and write the save back to disk.
/// Returns the Steam ID used so callers can re-encrypt other saves too.
export async function persistSession(session: SaveSession, steamId: string): Promise<void> {
  // Apply the ChangeSet to the SaveFile
  session.changes.apply(session.file);

  // Serialize to YAML bytes, encrypt, write
  const yamlBytes = session.file.toYaml();
  const encrypted = encryptSav(yamlBytes, steamId);

  const { writeFile } = await import('@tauri-apps/plugin-fs');
  await writeFile(session.info.path, encrypted);

  // Clear the mirror — changes are now saved
  session.changes.clear();
  for (const path of Object.keys(session.dirty)) {
    delete session.dirty[path];
  }
}
