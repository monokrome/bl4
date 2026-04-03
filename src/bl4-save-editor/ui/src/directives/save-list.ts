import { directive } from 'gonia';
import { requireContext, EditorKey, type EditorState } from '../contexts.js';

interface SaveListScope {
  displayName: (save: any) => string;
  select: (save: any) => void;
  isActive: (save: any) => boolean;
}

function displayName(classDisplayName: (raw: string | null) => string, save: any) {
  if (save.isProfile) return 'PROFILE';
  return save.characterName ?? classDisplayName(save.characterClass) ?? save.name.toUpperCase();
}

function select(editor: EditorState, save: any) {
  editor.activeSave = save.name;
  editor.activeSection = 'LOADOUT';
}

function isActive(editor: EditorState, save: any) {
  return editor.activeSave === save.name;
}

export function SaveListDirective($element: Element, $scope: SaveListScope & { classDisplayName: any }) {
  const editor = requireContext($element, EditorKey);

  Object.assign($scope, {
    displayName: displayName.bind(null, $scope.classDisplayName),
    select: select.bind(null, editor),
    isActive: isActive.bind(null, editor),
  });
}

directive('save-list', SaveListDirective, { scope: true });
