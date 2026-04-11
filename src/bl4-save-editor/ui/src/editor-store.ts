/// Editor store: a thin reactive wrapper over SaveFile + ChangeSet.
///
/// We keep a JS-side mirror of dirty paths alongside the Rust ChangeSet
/// because the WASM wrapper doesn't expose iteration or pending value
/// retrieval. Every mutation goes through here so the two stay in sync.

import { ChangeSet } from './bl4.js';
import type { EditorState, SaveSession, DirtyEntry } from './contexts.js';

/// Get the active session from editor state, or null if no save is selected.
export function activeSession(editor: EditorState): SaveSession | null {
  if (!editor.activeSave) return null;
  return editor.sessions[editor.activeSave] ?? null;
}

/// Read the current (saved) value at a path from the session's SaveFile.
/// Falls back to empty string if path doesn't exist.
export function readSaveValue(session: SaveSession, path: string): string {
  try {
    return session.file.get(path) ?? '';
  } catch {
    return '';
  }
}

/// Read the effective value: pending change if dirty, otherwise the saved value.
export function readEffectiveValue(session: SaveSession, path: string): string {
  const dirty = session.dirty[path];
  if (dirty !== undefined) return dirty.pending;
  return readSaveValue(session, path);
}

/// Check whether a path has a pending change.
export function isPathDirty(session: SaveSession, path: string): boolean {
  return session.dirty[path] !== undefined;
}

/// Write a change to a path. Updates both the Rust ChangeSet and the
/// reactive JS mirror. If the new value equals the original, the change
/// is cleared instead of recorded.
export function writeChange(session: SaveSession, path: string, value: string): void {
  const original = readSaveValue(session, path);

  if (value === original) {
    // Clearing — no-op if not dirty
    if (session.dirty[path] !== undefined) {
      session.changes.remove(path);
      delete session.dirty[path];
    }
    return;
  }

  // Record in Rust ChangeSet
  session.changes.add(path, value);

  // Update reactive mirror
  session.dirty[path] = { original, pending: value };
}

/// Revert a specific path, removing its entry from the ChangeSet.
export function revertPath(session: SaveSession, path: string): void {
  if (session.dirty[path] === undefined) return;
  session.changes.remove(path);
  delete session.dirty[path];
}

/// Revert all changes on a session.
export function revertAll(session: SaveSession): void {
  session.changes.clear();
  for (const path of Object.keys(session.dirty)) {
    delete session.dirty[path];
  }
}

/// List all dirty paths for a session.
export function dirtyPaths(session: SaveSession): string[] {
  return Object.keys(session.dirty).sort();
}

/// Count of pending changes on a session.
export function dirtyCount(session: SaveSession): number {
  return Object.keys(session.dirty).length;
}

/// Total dirty count across all sessions.
export function totalDirtyCount(editor: EditorState): number {
  let total = 0;
  for (const session of Object.values(editor.sessions)) {
    total += dirtyCount(session);
  }
  return total;
}

/// Any session with pending changes.
export function sessionsWithChanges(editor: EditorState): SaveSession[] {
  return Object.values(editor.sessions).filter(s => dirtyCount(s) > 0);
}

/// Dispose a session's SaveFile + ChangeSet. Called when closing a save.
export function disposeSession(session: SaveSession): void {
  try { session.file.free(); } catch {}
  try { session.changes.free(); } catch {}
  for (const path of Object.keys(session.dirty)) {
    delete session.dirty[path];
  }
}

/// Create a new empty ChangeSet for a session.
export function createChangeSet(): ChangeSet {
  return new ChangeSet();
}

/// Export a DirtyEntry type re-export for convenience.
export type { DirtyEntry };
