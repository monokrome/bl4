import { directive, reactive, registerContext, createEffectScope } from 'gonia';
import { exists, readDir } from '@tauri-apps/plugin-fs';
import { homeDir } from '@tauri-apps/api/path';
import { SteamIdContext, EditorContext, type EditorState } from '../contexts.js';
import { loadDirectory } from '../saves.js';

const CLASS_NAMES: Record<string, string> = {
  Char_DarkSiren: 'Vex',
  Char_RoboDealer: 'C4SH',
  Char_Paladin: 'Amon',
  Char_ExoSoldier: 'Rafa',
  Char_Gravitar: 'Harlowe',
};

interface AppRootScope {
  editor: EditorState;
  steamId: { value: string };
  classDisplayName: (raw: string | null) => string;
  closeDrawer: () => void;
}

function classDisplayName(raw: string | null): string {
  if (!raw) return 'Unknown';
  return CLASS_NAMES[raw] ?? raw.replace('Char_', '');
}

export function AppRootDirective($element: Element, $scope: AppRootScope) {
  const effectScope = createEffectScope();

  const steamId = reactive({ value: '' });
  const editor: EditorState = reactive({
    saves: [],
    activeSave: null,
    activeSection: null,
    drawerOpen: false,
    loading: false,
    error: null,
  });

  registerContext($element, SteamIdContext, steamId);
  registerContext($element, EditorContext, editor);

  Object.assign($scope, {
    editor,
    steamId,
    classDisplayName,
    closeDrawer: () => { editor.drawerOpen = false; },
  });

  detectAndLoad(steamId, editor);

  return () => effectScope.stop();
}

directive('app-root', AppRootDirective, { scope: true });

const SAVE_BASES = [
  '.local/share/Steam/steamapps/compatdata/1285190/pfx/drive_c/users/steamuser/Documents/My Games/Borderlands 4/Saved/SaveGames',
  'Documents/My Games/Borderlands 4/Saved/SaveGames',
];

function joinPath(base: string, ...segments: string[]): string {
  const sep = base.includes('\\') ? '\\' : '/';
  const trimmed = base.replace(/[\\/]+$/, '');
  return [trimmed, ...segments].join(sep);
}

async function detectAndLoad(steamId: { value: string }, editor: EditorState) {
  try {
    const home = await homeDir();
    for (const base of SAVE_BASES) {
      const savePath = joinPath(home, ...base.split('/'));
      if (!await exists(savePath)) continue;

      const entries = await readDir(savePath);
      for (const entry of entries) {
        if (entry.name && /^\d{17}$/.test(entry.name)) {
          steamId.value = entry.name;
          const saveDir = joinPath(savePath, entry.name, 'Profiles', 'client');
          await loadDirectory(saveDir, entry.name, editor, classDisplayName);
          return;
        }
      }
    }
  } catch {
    // Auto-detection is best-effort
  }
}
