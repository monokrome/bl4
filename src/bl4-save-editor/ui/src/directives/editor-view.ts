import { directive } from 'gonia';
import { requireContext, EditorKey, type EditorState } from '../contexts.js';

interface EditorViewScope {
  sections: string[];
  selectSection: (section: string) => void;
  isSectionActive: (section: string) => boolean;
}

const SECTIONS = ['LOADOUT', 'BACKPACK', 'SKILLS', 'SPECIALIZATIONS', 'SDUs', 'STATS', 'MAP'];

function selectSection(editor: EditorState, section: string) {
  editor.activeSection = section;
}

function isSectionActive(editor: EditorState, section: string) {
  return editor.activeSection === section;
}

export function EditorViewDirective($element: Element, $scope: EditorViewScope) {
  const editor = requireContext($element, EditorKey);

  Object.assign($scope, {
    sections: SECTIONS,
    selectSection: selectSection.bind(null, editor),
    isSectionActive: isSectionActive.bind(null, editor),
  });
}

directive('editor-view', EditorViewDirective, {
  scope: true,
});