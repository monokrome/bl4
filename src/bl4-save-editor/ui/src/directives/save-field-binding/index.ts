import { directive } from 'gonia';
import template from './template.pug';
import { EditorContext, type EditorState, type SaveSession } from '../../contexts.js';
import {
  readEffectiveValue,
  isPathDirty,
  writeChange,
  activeSession,
} from '../../editor-store.js';

/// The reactive-ish binding object that templates consume.
/// Backed by getters so each read walks the editor store and picks
/// up whatever session is currently active.
export interface FieldBinding {
  readonly value: string;
  readonly dirty: boolean;
  readonly originalValue: string;
  onChange: (next: string) => void;
}

/// Resolve the session to bind against: profile if `profile` is set,
/// otherwise the currently active character save.
function resolveSession(editor: EditorState, useProfile: boolean): SaveSession | null {
  if (useProfile) {
    const profileInfo = editor.saves.find(s => s.isProfile);
    if (!profileInfo) return null;
    return editor.sessions[profileInfo.name] ?? null;
  }
  return activeSession(editor);
}

/// Build the binding object for a specific path. The getters read
/// from the live editor store, so the template reactively updates
/// whenever the session or its dirty mirror changes.
function createBinding(
  editor: EditorState,
  path: string,
  useProfile: boolean,
): FieldBinding {
  return {
    get value(): string {
      const session = resolveSession(editor, useProfile);
      if (!session) return '';
      return readEffectiveValue(session, path);
    },
    get dirty(): boolean {
      const session = resolveSession(editor, useProfile);
      if (!session) return false;
      return isPathDirty(session, path);
    },
    get originalValue(): string {
      const session = resolveSession(editor, useProfile);
      if (!session) return '';
      try {
        return session.file.get(path) ?? '';
      } catch {
        return '';
      }
    },
    onChange(next: string) {
      const session = resolveSession(editor, useProfile);
      if (!session) return;
      writeChange(session, path, next);
    },
  };
}

interface SaveFieldBindingScope {
  [key: string]: unknown;
}

/// Reads `path`, `as`, and optional `profile` attributes from the
/// element, then publishes the binding on its scope under the name
/// given by `as`. Descendant templates reference it directly.
export function SaveFieldBindingDirective(
  $element: Element,
  $scope: SaveFieldBindingScope,
  editor: EditorState,
) {
  const path = $element.getAttribute('path') ?? '';
  const as = $element.getAttribute('as') ?? 'field';
  const profile = $element.getAttribute('profile') === 'true';

  if (!path || !as) return;

  $scope[as] = createBinding(editor, path, profile);
}
SaveFieldBindingDirective.$inject = ['$element', '$scope'];

directive('save-field-binding', SaveFieldBindingDirective, {
  scope: true,
  template,
  using: [EditorContext],
});
