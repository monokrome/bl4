import { directive } from 'gonia';
import { EditorContext, type EditorState } from '../contexts.js';

interface EditorViewScope {
  sections: () => string[];
  activeSaveName: () => string;
  selectSection: (section: string) => void;
  isSectionActive: (section: string) => boolean;
  classDisplayName: (raw: string | null) => string;
}

const CHARACTER_SECTIONS = ['LOADOUT', 'BACKPACK', 'SKILLS', 'SPECIALIZATIONS', 'SDUs', 'STATS', 'MAP'];
const PROFILE_SECTIONS = ['BANK', 'UNLOCKABLES'];

function activeSaveName(editor: EditorState, classDisplayName: (raw: string | null) => string): string {
  const save = editor.saves.find(s => s.name === editor.activeSave);
  if (!save) return '';
  if (save.isProfile) return 'PROFILE';
  if (save.characterName) return save.characterName;
  if (save.characterClass) return classDisplayName(save.characterClass);
  return save.name;
}

function selectSection(editor: EditorState, section: string) {
  editor.activeSection = section;
}

function isSectionActive(editor: EditorState, section: string) {
  return editor.activeSection === section;
}

export function EditorViewDirective($scope: EditorViewScope, editor: EditorState) {
  function sections(): string[] {
    const save = editor.saves.find(s => s.name === editor.activeSave);
    return save?.isProfile ? PROFILE_SECTIONS : CHARACTER_SECTIONS;
  }

  Object.assign($scope, {
    sections,
    activeSaveName: activeSaveName.bind(null, editor, $scope.classDisplayName),
    selectSection: selectSection.bind(null, editor),
    isSectionActive: isSectionActive.bind(null, editor),
  });
}
EditorViewDirective.$inject = ['$scope'];

directive('editor-view', EditorViewDirective, { scope: true, using: [EditorContext] });
