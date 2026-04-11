/// Test harness for Gonia-backed directives.
///
/// Creates an isolated DOM subtree, registers any contexts the test
/// needs (EditorContext, SteamIdContext), hydrates the subtree, and
/// exposes helpers for interacting with it. Gonia's public API only
/// exposes `hydrate`, `clearRootScope`, and `clearContexts` for test
/// support; anything more invasive isn't reachable through the
/// package export map, so the harness sticks to those.

import { hydrate } from 'gonia/client';
import {
  clearRootScope,
  clearContexts,
  registerContext,
  reactive,
  type ContextKey,
} from 'gonia';
import { EditorContext, SteamIdContext, type EditorState } from './contexts.js';

/// Options for `mountHTML`.
export interface MountOptions {
  editor?: EditorState;
  steamId?: { value: string };
  contexts?: Array<[ContextKey<unknown>, unknown]>;
}

/// A hydrated subtree with helpers for manipulating/inspecting it.
export interface Mounted {
  /// The root element holding the rendered subtree.
  root: HTMLElement;
  /// The editor state used for the mount (reactive).
  editor: EditorState;
  /// Find a single element inside root.
  find<E extends Element = Element>(selector: string): E | null;
  /// Find all matching elements inside root.
  findAll<E extends Element = Element>(selector: string): E[];
  /// Simulate an input event on an input with a new value.
  typeInto(selector: string, value: string): void;
  /// Remove the mount from the DOM.
  destroy(): void;
}

const activeMounts: Mounted[] = [];

/// Build a blank reactive EditorState for tests that don't need
/// a specific session loaded.
export function makeEmptyEditor(): EditorState {
  return reactive({
    saves: [],
    sessions: {},
    activeSave: null,
    activeSection: null,
    drawerOpen: false,
    loading: false,
    error: null,
    skipSaveConfirm: false,
  }) as EditorState;
}

/// Mount an HTML string, register contexts, and hydrate it.
export async function mountHTML(html: string, opts: MountOptions = {}): Promise<Mounted> {
  const editor = opts.editor ?? makeEmptyEditor();
  const steamId = opts.steamId ?? (reactive({ value: '' }) as { value: string });

  const root = document.createElement('div');
  root.setAttribute('data-test-root', '');
  root.innerHTML = html;
  document.body.appendChild(root);

  registerContext(root, EditorContext, editor);
  registerContext(root, SteamIdContext, steamId);
  for (const [key, value] of opts.contexts ?? []) {
    registerContext(root, key, value);
  }

  await hydrate(root);

  const mount: Mounted = {
    root,
    editor,
    find<E extends Element = Element>(selector: string): E | null {
      return root.querySelector<E>(selector);
    },
    findAll<E extends Element = Element>(selector: string): E[] {
      return Array.from(root.querySelectorAll<E>(selector));
    },
    typeInto(selector: string, value: string) {
      const el = root.querySelector<HTMLInputElement>(selector);
      if (!el) throw new Error(`typeInto: no element matched ${selector}`);
      el.value = value;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    destroy() {
      const idx = activeMounts.indexOf(mount);
      if (idx >= 0) activeMounts.splice(idx, 1);
      root.remove();
    },
  };

  activeMounts.push(mount);
  return mount;
}

/// Register a Vitest `afterEach` hook that cleans up lingering
/// mounts and resets shared Gonia state between tests. Call once
/// at the top of a test file that uses `mountHTML`.
export function setupMountTeardown(afterEach: (fn: () => void) => void): void {
  afterEach(() => {
    while (activeMounts.length) {
      const mount = activeMounts.pop();
      try {
        mount?.root.remove();
      } catch {
        // best-effort
      }
    }
    clearRootScope();
    clearContexts();
  });
}
