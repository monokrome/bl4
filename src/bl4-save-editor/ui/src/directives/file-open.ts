import { directive } from 'gonia';
import { open } from '@tauri-apps/plugin-dialog';
import { SteamIdContext, EditorContext, type EditorState } from '../contexts.js';
import { extractSteamIdFromPath, loadDirectory, loadSingleFile } from '../saves.js';

interface FileOpenScope {
  showFileOpen: () => boolean;
  updateSteamId: (e: Event) => void;
  openDir: () => Promise<void>;
  openFile: () => Promise<void>;
  classDisplayName: (raw: string | null) => string;
}

function resolveSteamId(path: string, inputValue: string): string | null {
  return extractSteamIdFromPath(path) ?? (inputValue || null);
}

export function FileOpenDirective($element: Element, $scope: FileOpenScope, steamId: { value: string }, editor: EditorState) {
  Object.assign($scope, {
    showFileOpen: () => editor.saves.length === 0 && !editor.activeSave && !editor.loading,

    updateSteamId: (e: Event) => {
      steamId.value = (e.target as HTMLInputElement).value.trim();
    },

    openDir: async () => {
      const dir = await open({ directory: true, title: 'Open Save Directory' });
      if (!dir) return;

      const sid = resolveSteamId(dir, steamId.value);
      if (!sid) { editor.error = 'Could not determine Steam ID. Enter it manually.'; return; }
      steamId.value = sid;
      await loadDirectory(dir, sid, editor, $scope.classDisplayName);
    },

    openFile: async () => {
      const path = await open({
        title: 'Open Save File',
        filters: [{ name: 'Save Files', extensions: ['sav'] }],
      });
      if (!path) return;

      const sid = resolveSteamId(path, steamId.value);
      if (!sid) { editor.error = 'Could not determine Steam ID. Enter it manually.'; return; }
      steamId.value = sid;
      await loadSingleFile(path, sid, editor, $scope.classDisplayName);
    },
  });
}
FileOpenDirective.$inject = ['$element', '$scope'];

directive('file-open', FileOpenDirective, { scope: true, using: [SteamIdContext, EditorContext] });
