import { directive } from 'gonia';
import { requireContext, EditorKey, type EditorState } from '../contexts.js';

interface AppHeaderScope {
  toggleDrawer: () => void;
  drawerIcon: () => string;
}

function toggleDrawer(editor: EditorState) {
  editor.drawerOpen = !editor.drawerOpen;
}

function drawerIcon(editor: EditorState) {
  return editor.drawerOpen ? '>' : '<';
}

export function AppHeaderDirective($element: Element, $scope: AppHeaderScope) {
  const editor = requireContext($element, EditorKey);

  Object.assign($scope, {
    toggleDrawer: toggleDrawer.bind(null, editor),
    drawerIcon: drawerIcon.bind(null, editor),
  });
}

directive('app-header', AppHeaderDirective, { scope: true });
