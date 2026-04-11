import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  readSaveValue,
  readEffectiveValue,
  isPathDirty,
  writeChange,
  revertPath,
  revertAll,
  dirtyPaths,
  dirtyCount,
  totalDirtyCount,
  sessionsWithChanges,
  activeSession,
} from './editor-store.js';
import type { EditorState, SaveSession, SaveInfo } from './contexts.js';

/// A minimal mock of the SaveFile WASM class that satisfies the
/// subset of the API `editor-store.ts` touches.
function makeMockFile(initial: Record<string, string>) {
  const data = { ...initial };
  return {
    get: vi.fn((path: string) => data[path]),
    set: vi.fn((path: string, value: string) => {
      data[path] = value;
    }),
    _data: data,
  } as any;
}

/// A minimal mock of the ChangeSet WASM class, tracking calls so
/// we can verify the store syncs them correctly.
function makeMockChangeSet() {
  const calls: Array<{ op: string; args: unknown[] }> = [];
  return {
    add: vi.fn((path: string, value: string) => {
      calls.push({ op: 'add', args: [path, value] });
    }),
    remove: vi.fn((path: string) => {
      calls.push({ op: 'remove', args: [path] });
      return true;
    }),
    clear: vi.fn(() => {
      calls.push({ op: 'clear', args: [] });
    }),
    free: vi.fn(),
    _calls: calls,
  } as any;
}

function makeSession(name: string, isProfile: boolean, initial: Record<string, string>): SaveSession {
  const info: SaveInfo = {
    path: `/tmp/${name}.sav`,
    name,
    isProfile,
    characterName: null,
    characterClass: null,
    level: null,
    difficulty: null,
    hint: '',
  };
  return {
    info,
    file: makeMockFile(initial),
    changes: makeMockChangeSet(),
    dirty: {},
  };
}

function makeEditor(...sessions: SaveSession[]): EditorState {
  const state: EditorState = {
    saves: sessions.map(s => s.info),
    sessions: Object.fromEntries(sessions.map(s => [s.info.name, s])),
    activeSave: sessions[0]?.info.name ?? null,
    activeSection: null,
    drawerOpen: false,
    loading: false,
    error: null,
    skipSaveConfirm: false,
  };
  return state;
}

describe('editor-store', () => {
  describe('readSaveValue', () => {
    it('returns the value from the SaveFile at a given path', () => {
      const session = makeSession('s1', false, { 'state.currencies.cash': '1000' });
      expect(readSaveValue(session, 'state.currencies.cash')).toBe('1000');
    });

    it('returns an empty string for missing paths', () => {
      const session = makeSession('s1', false, {});
      expect(readSaveValue(session, 'state.currencies.cash')).toBe('');
    });

    it('swallows SaveFile.get exceptions and returns empty', () => {
      const session = makeSession('s1', false, {});
      (session.file.get as any) = vi.fn(() => {
        throw new Error('nope');
      });
      expect(readSaveValue(session, 'any.path')).toBe('');
    });
  });

  describe('writeChange / readEffectiveValue / isPathDirty', () => {
    it('records a pending change and reflects it in effective value + dirty state', () => {
      const session = makeSession('s1', false, { 'state.currencies.cash': '1000' });

      expect(isPathDirty(session, 'state.currencies.cash')).toBe(false);
      expect(readEffectiveValue(session, 'state.currencies.cash')).toBe('1000');

      writeChange(session, 'state.currencies.cash', '9999');

      expect(isPathDirty(session, 'state.currencies.cash')).toBe(true);
      expect(readEffectiveValue(session, 'state.currencies.cash')).toBe('9999');
      expect(session.changes.add).toHaveBeenCalledWith('state.currencies.cash', '9999');
    });

    it('clears the change when the new value equals the original', () => {
      const session = makeSession('s1', false, { 'state.currencies.cash': '1000' });

      writeChange(session, 'state.currencies.cash', '9999');
      expect(isPathDirty(session, 'state.currencies.cash')).toBe(true);

      writeChange(session, 'state.currencies.cash', '1000');
      expect(isPathDirty(session, 'state.currencies.cash')).toBe(false);
      expect(session.changes.remove).toHaveBeenCalledWith('state.currencies.cash');
      expect(session.dirty['state.currencies.cash']).toBeUndefined();
    });

    it('does not record or remove when value unchanged and not already dirty', () => {
      const session = makeSession('s1', false, { 'state.currencies.cash': '1000' });
      writeChange(session, 'state.currencies.cash', '1000');
      expect(session.changes.add).not.toHaveBeenCalled();
      expect(session.changes.remove).not.toHaveBeenCalled();
    });

    it('captures the original value on the first write, not subsequent ones', () => {
      const session = makeSession('s1', false, { 'a.path': 'orig' });

      writeChange(session, 'a.path', 'first');
      expect(session.dirty['a.path']?.original).toBe('orig');

      writeChange(session, 'a.path', 'second');
      expect(session.dirty['a.path']?.original).toBe('orig');
      expect(session.dirty['a.path']?.pending).toBe('second');
    });
  });

  describe('revertPath', () => {
    it('removes a single dirty entry and calls changes.remove', () => {
      const session = makeSession('s1', false, { 'p1': 'a', 'p2': 'b' });
      writeChange(session, 'p1', 'A');
      writeChange(session, 'p2', 'B');

      revertPath(session, 'p1');

      expect(isPathDirty(session, 'p1')).toBe(false);
      expect(isPathDirty(session, 'p2')).toBe(true);
      expect(session.changes.remove).toHaveBeenCalledWith('p1');
    });

    it('is a no-op when the path is not dirty', () => {
      const session = makeSession('s1', false, { 'p1': 'a' });
      const before = (session.changes as any)._calls.length;
      revertPath(session, 'p1');
      expect((session.changes as any)._calls.length).toBe(before);
    });
  });

  describe('revertAll', () => {
    it('clears every dirty entry and calls changes.clear', () => {
      const session = makeSession('s1', false, { 'p1': 'a', 'p2': 'b', 'p3': 'c' });
      writeChange(session, 'p1', 'A');
      writeChange(session, 'p2', 'B');
      writeChange(session, 'p3', 'C');
      expect(dirtyCount(session)).toBe(3);

      revertAll(session);

      expect(dirtyCount(session)).toBe(0);
      expect(session.changes.clear).toHaveBeenCalled();
      expect(dirtyPaths(session)).toEqual([]);
    });
  });

  describe('dirtyPaths / dirtyCount', () => {
    it('returns sorted dirty paths and their count', () => {
      const session = makeSession('s1', false, { 'zeta': '1', 'alpha': '2', 'mu': '3' });
      writeChange(session, 'zeta', 'Z');
      writeChange(session, 'alpha', 'A');
      writeChange(session, 'mu', 'M');

      expect(dirtyPaths(session)).toEqual(['alpha', 'mu', 'zeta']);
      expect(dirtyCount(session)).toBe(3);
    });

    it('returns empty list for a clean session', () => {
      const session = makeSession('s1', false, {});
      expect(dirtyPaths(session)).toEqual([]);
      expect(dirtyCount(session)).toBe(0);
    });
  });

  describe('totalDirtyCount / sessionsWithChanges', () => {
    it('aggregates dirty counts across all sessions', () => {
      const s1 = makeSession('1', false, { 'p': 'a' });
      const s2 = makeSession('2', false, { 'p': 'a', 'q': 'b' });
      const s3 = makeSession('profile', true, {});

      writeChange(s1, 'p', 'x');
      writeChange(s2, 'p', 'y');
      writeChange(s2, 'q', 'z');

      const editor = makeEditor(s1, s2, s3);

      expect(totalDirtyCount(editor)).toBe(3);
      expect(sessionsWithChanges(editor)).toHaveLength(2);
      expect(sessionsWithChanges(editor).map(s => s.info.name).sort())
        .toEqual(['1', '2']);
    });
  });

  describe('activeSession', () => {
    it('returns the session matching editor.activeSave', () => {
      const s1 = makeSession('char1', false, {});
      const s2 = makeSession('char2', false, {});
      const editor = makeEditor(s1, s2);
      editor.activeSave = 'char2';
      expect(activeSession(editor)).toBe(s2);
    });

    it('returns null when no active save', () => {
      const editor = makeEditor();
      expect(activeSession(editor)).toBeNull();
    });

    it('returns null when active save does not exist in sessions', () => {
      const editor = makeEditor();
      editor.activeSave = 'phantom';
      expect(activeSession(editor)).toBeNull();
    });
  });
});
