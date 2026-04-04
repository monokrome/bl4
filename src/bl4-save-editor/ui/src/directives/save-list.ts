import { directive } from 'gonia';
import { EditorContext, type EditorState, type SaveInfo } from '../contexts.js';

const CLASS_ARCHETYPES: Record<string, string> = {
  Char_DarkSiren: 'Siren',
  Char_RoboDealer: 'Dealer',
  Char_Paladin: 'Paladin',
  Char_ExoSoldier: 'Soldier',
  Char_Gravitar: 'Gravitar',
};

interface SaveListScope {
  isCollapsed: () => boolean;
  nameText: (save: SaveInfo) => string;
  attrText: (save: SaveInfo) => string;
  levelText: (save: SaveInfo) => string;
  fileName: (save: SaveInfo) => string;
  select: (save: SaveInfo) => void;
  classDisplayName: (raw: string | null) => string;
}

function nameText(classDisplayName: (raw: string | null) => string, save: SaveInfo): string {
  if (save.isProfile) return 'PROFILE';
  if (save.characterName) return save.characterName;
  if (save.characterClass) return classDisplayName(save.characterClass);
  return save.name.toUpperCase();
}

function attrText(save: SaveInfo): string {
  if (save.isProfile) return 'Bank · Unlockables';
  const archetype = save.characterClass ? CLASS_ARCHETYPES[save.characterClass] ?? null : null;
  const parts = [archetype, save.difficulty].filter(Boolean);
  return parts.join(' · ');
}

function levelText(save: SaveInfo): string {
  if (save.level === null) return '';
  return `Level ${save.level}`;
}

function fileName(save: SaveInfo): string {
  return save.name.endsWith('.sav') ? save.name : `${save.name}.sav`;
}

function select(editor: EditorState, save: SaveInfo) {
  editor.activeSave = save.name;
  editor.activeSection = save.isProfile ? 'BANK' : 'LOADOUT';
}

export function SaveListDirective($element: Element, $scope: SaveListScope, editor: EditorState) {
  Object.assign($scope, {
    isCollapsed: () => editor.activeSave !== null,
    nameText: nameText.bind(null, $scope.classDisplayName),
    attrText,
    levelText,
    fileName,
    select: select.bind(null, editor),
  });
}
SaveListDirective.$inject = ['$element', '$scope'];

directive('save-list', SaveListDirective, { scope: true, using: [EditorContext] });
