import { createContextKey } from 'gonia';
import type { SaveFile, ChangeSet } from './bl4.js';

export interface SaveInfo {
  path: string;
  name: string;
  isProfile: boolean;
  characterName: string | null;
  characterClass: string | null;
  level: number | null;
  difficulty: string | null;
  hint: string;
}

/// A dirty entry tracked in the JS-side mirror of a ChangeSet.
/// We keep this alongside the Rust ChangeSet because the WASM wrapper
/// doesn't expose iteration or pending value retrieval.
export interface DirtyEntry {
  /// The original value at this path, before the change.
  original: string;
  /// The new value that will be written.
  pending: string;
}

/// Per-save editing session: the loaded SaveFile, its ChangeSet, and the JS
/// mirror of dirty entries. Kept in EditorState alongside SaveInfo.
export interface SaveSession {
  info: SaveInfo;
  /// Decrypted SaveFile (kept alive so we can read live values).
  file: SaveFile;
  /// ChangeSet that pending edits are batched into.
  changes: ChangeSet;
  /// JS mirror: path -> { original, pending }. Reactive.
  dirty: Record<string, DirtyEntry>;
}

export interface EditorState {
  /// Lightweight info rows shown in the save list.
  saves: SaveInfo[];
  /// Full sessions keyed by SaveInfo.name.
  sessions: Record<string, SaveSession>;
  /// Name of the currently active save (key into `sessions`).
  activeSave: string | null;
  /// Current section/tab inside the active save.
  activeSection: string | null;
  drawerOpen: boolean;
  loading: boolean;
  error: string | null;
  /// User preference: skip save confirmation dialog.
  skipSaveConfirm: boolean;
}

export const SteamIdContext = createContextKey<{ value: string }>('steamId');
export const EditorContext = createContextKey<EditorState>('editor');
