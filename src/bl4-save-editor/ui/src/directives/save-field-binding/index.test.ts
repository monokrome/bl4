import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { EditorState, SaveSession, SaveInfo } from '../../contexts.js';
import { SaveFieldBindingDirective, type FieldBinding } from './index.js';

/// Minimal SaveFile/ChangeSet stubs matching the ones in editor-store.test.ts.
function makeMockFile(initial: Record<string, string>) {
  const data = { ...initial };
  return {
    get: vi.fn((path: string) => data[path]),
    set: vi.fn((path: string, value: string) => {
      data[path] = value;
    }),
  } as any;
}

function makeMockChangeSet() {
  return {
    add: vi.fn(),
    remove: vi.fn(() => true),
    clear: vi.fn(),
    free: vi.fn(),
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
  return {
    saves: sessions.map(s => s.info),
    sessions: Object.fromEntries(sessions.map(s => [s.info.name, s])),
    activeSave: sessions.find(s => !s.info.isProfile)?.info.name ?? null,
    activeSection: null,
    drawerOpen: false,
    loading: false,
    error: null,
    skipSaveConfirm: false,
  };
}

/// Construct a fake element that serves attribute queries from a map.
function makeElement(attrs: Record<string, string>): Element {
  return {
    getAttribute: (name: string) => attrs[name] ?? null,
  } as unknown as Element;
}

/// Runs the directive function against a scope + editor to produce
/// the binding, then returns the binding for assertions.
function runBindingDirective(
  editor: EditorState,
  attrs: Record<string, string>,
): FieldBinding | undefined {
  const scope: Record<string, unknown> = {};
  const el = makeElement(attrs);
  SaveFieldBindingDirective(el, scope, editor);
  return scope[attrs.as] as FieldBinding | undefined;
}

describe('save-field-binding', () => {
  let editor: EditorState;
  let character: SaveSession;
  let profile: SaveSession;

  beforeEach(() => {
    character = makeSession('char1', false, {
      'state.currencies.cash': '1000',
      'state.char_name': 'Vex',
    });
    profile = makeSession('profile', true, {
      'inventory.items.bank.slot_0.serial': '@Ug...',
    });
    editor = makeEditor(character, profile);
  });

  describe('binding publication', () => {
    it('publishes a binding on scope under the `as` key', () => {
      const binding = runBindingDirective(editor, {
        path: 'state.currencies.cash',
        as: 'cash',
      });

      expect(binding).toBeDefined();
      expect(typeof binding!.onChange).toBe('function');
    });

    it('omits publication when path or as is missing', () => {
      const scope: Record<string, unknown> = {};
      SaveFieldBindingDirective(makeElement({ path: '', as: 'cash' }), scope, editor);
      expect(scope.cash).toBeUndefined();

      SaveFieldBindingDirective(makeElement({ path: 'a.b', as: '' }), scope, editor);
      expect(Object.keys(scope)).toHaveLength(0);
    });
  });

  describe('value / originalValue / dirty getters', () => {
    it('reflects the session value when no pending change', () => {
      const binding = runBindingDirective(editor, {
        path: 'state.currencies.cash',
        as: 'cash',
      })!;

      expect(binding.value).toBe('1000');
      expect(binding.originalValue).toBe('1000');
      expect(binding.dirty).toBe(false);
    });

    it('returns empty string when active session is null', () => {
      editor.activeSave = null;
      const binding = runBindingDirective(editor, {
        path: 'state.currencies.cash',
        as: 'cash',
      })!;

      expect(binding.value).toBe('');
      expect(binding.dirty).toBe(false);
    });

    it('reflects pending changes after onChange', () => {
      const binding = runBindingDirective(editor, {
        path: 'state.currencies.cash',
        as: 'cash',
      })!;

      binding.onChange('9999');

      expect(binding.value).toBe('9999');
      expect(binding.originalValue).toBe('1000');
      expect(binding.dirty).toBe(true);
      expect(character.changes.add).toHaveBeenCalledWith('state.currencies.cash', '9999');
    });

    it('clears dirty when the change is reverted by writing the original', () => {
      const binding = runBindingDirective(editor, {
        path: 'state.currencies.cash',
        as: 'cash',
      })!;

      binding.onChange('9999');
      expect(binding.dirty).toBe(true);

      binding.onChange('1000');
      expect(binding.dirty).toBe(false);
      expect(binding.value).toBe('1000');
    });
  });

  describe('profile binding', () => {
    it('resolves to the profile session when profile="true"', () => {
      const binding = runBindingDirective(editor, {
        path: 'inventory.items.bank.slot_0.serial',
        as: 'bankItem',
        profile: 'true',
      })!;

      expect(binding.value).toBe('@Ug...');
    });

    it('defaults to active character when profile is absent', () => {
      const binding = runBindingDirective(editor, {
        path: 'state.char_name',
        as: 'name',
      })!;

      expect(binding.value).toBe('Vex');
    });

    it('returns empty string when no profile session exists', () => {
      editor = makeEditor(character); // no profile
      const binding = runBindingDirective(editor, {
        path: 'inventory.items.bank.slot_0.serial',
        as: 'bankItem',
        profile: 'true',
      })!;

      expect(binding.value).toBe('');
    });
  });

  describe('reactivity across active save changes', () => {
    it('getters re-resolve the session on every read', () => {
      const char2 = makeSession('char2', false, {
        'state.currencies.cash': '500',
      });
      editor.sessions[char2.info.name] = char2;
      editor.saves.push(char2.info);

      const binding = runBindingDirective(editor, {
        path: 'state.currencies.cash',
        as: 'cash',
      })!;

      expect(binding.value).toBe('1000'); // char1

      editor.activeSave = 'char2';
      expect(binding.value).toBe('500'); // char2 after switch
    });
  });
});
