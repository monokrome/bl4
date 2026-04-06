import { directive } from 'gonia';
import { EditorContext, type EditorState, type SaveInfo } from '../contexts.js';

interface SaveListScope {
  isCollapsed: () => boolean;
  displayName: (save: SaveInfo) => string;
  displayHint: (save: SaveInfo) => string;
  select: (save: SaveInfo) => void;
  classDisplayName: (raw: string | null) => string;
}

function displayName(classDisplayName: (raw: string | null) => string, save: SaveInfo): string {
  if (save.isProfile) return 'PROFILE';
  return save.characterName ?? classDisplayName(save.characterClass) ?? save.name.toUpperCase();
}

function displayHint(save: SaveInfo): string {
  return save.hint;
}

function select(editor: EditorState, save: SaveInfo) {
  editor.activeSave = save.name;
  editor.activeSection = save.isProfile ? 'BANK' : 'LOADOUT';
}

export function SaveListDirective($scope: SaveListScope, editor: EditorState) {
  Object.assign($scope, {
    isCollapsed: () => editor.activeSave !== null,
    displayName: displayName.bind(null, $scope.classDisplayName),
    displayHint,
    select: select.bind(null, editor),
  });
}
SaveListDirective.$inject = ['$scope'];

directive('save-list', SaveListDirective, { scope: true, using: [EditorContext] });
