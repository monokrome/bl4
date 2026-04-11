/// Integration tests for character-cash: exercises the full pipeline
/// of withSaveField → save-field-binding → pug template → hydrate →
/// reactive DOM updates. Uses the public Gonia test helpers via
/// the shared mount harness.

import { describe, it, expect, afterEach, vi } from 'vitest';
import { mountHTML, setupMountTeardown, makeEmptyEditor } from '../../test-utils.js';
import type { SaveSession, SaveInfo } from '../../contexts.js';

// Registers directives as a side-effect of import.
import '../save-field-binding/index.js';
import '../labeled-field/index.js';
import './index.js';

setupMountTeardown(afterEach);

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

function makeSession(name: string, initial: Record<string, string>): SaveSession {
  const info: SaveInfo = {
    path: `/tmp/${name}.sav`,
    name,
    isProfile: false,
    characterName: 'Vex',
    characterClass: 'Char_DarkSiren',
    level: 60,
    difficulty: 'Normal',
    hint: 'Lv60 Vex / Normal',
  };
  return {
    info,
    file: makeMockFile(initial),
    changes: makeMockChangeSet(),
    dirty: {},
  };
}

describe('character-cash (integration)', () => {
  it('renders the current cash value from the active session', async () => {
    const editor = makeEmptyEditor();
    const session = makeSession('char1', { 'state.currencies.cash': '1234' });
    editor.saves = [session.info];
    editor.sessions[session.info.name] = session;
    editor.activeSave = session.info.name;

    const mount = await mountHTML('<character-cash></character-cash>', { editor });
    const input = mount.find<HTMLInputElement>('input[type="number"]');

    expect(input).not.toBeNull();
    expect(input!.value).toBe('1234');
  });

  it('writes changes to the ChangeSet on input events', async () => {
    const editor = makeEmptyEditor();
    const session = makeSession('char1', { 'state.currencies.cash': '100' });
    editor.saves = [session.info];
    editor.sessions[session.info.name] = session;
    editor.activeSave = session.info.name;

    const mount = await mountHTML('<character-cash></character-cash>', { editor });

    mount.typeInto('input[type="number"]', '9999');

    expect(session.changes.add).toHaveBeenCalledWith('state.currencies.cash', '9999');
    expect(session.dirty['state.currencies.cash']).toEqual({
      original: '100',
      pending: '9999',
    });
  });

  it('reveals the inline dirty marker after a change', async () => {
    const editor = makeEmptyEditor();
    const session = makeSession('char1', { 'state.currencies.cash': '100' });
    editor.saves = [session.info];
    editor.sessions[session.info.name] = session;
    editor.activeSave = session.info.name;

    const mount = await mountHTML('<character-cash></character-cash>', { editor });

    // The marker is a span inside the character-cash template, gated
    // by g-show on cash.dirty — hidden at mount.
    const before = mount.find<HTMLElement>('span[title="Pending change"]');
    expect(before).not.toBeNull();
    expect(before!.style.display).toBe('none');

    mount.typeInto('input[type="number"]', '500');
    await Promise.resolve();

    const after = mount.find<HTMLElement>('span[title="Pending change"]');
    expect(after!.style.display).not.toBe('none');
  });

  it('renders empty when no session is active', async () => {
    const editor = makeEmptyEditor();
    // No sessions loaded, activeSave remains null
    const mount = await mountHTML('<character-cash></character-cash>', { editor });
    const input = mount.find<HTMLInputElement>('input[type="number"]');
    expect(input).not.toBeNull();
    expect(input!.value).toBe('');
  });
});
