import { directive } from 'gonia';
import { EditorContext, type EditorState } from '../contexts.js';

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

export function AppHeaderDirective($scope: AppHeaderScope, editor: EditorState) {
  Object.assign($scope, {
    toggleDrawer: toggleDrawer.bind(null, editor),
    drawerIcon: drawerIcon.bind(null, editor),
  });
}
AppHeaderDirective.$inject = ['$scope'];

directive('app-header', AppHeaderDirective, { scope: true, using: [EditorContext] });
