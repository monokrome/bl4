import { directive, reactive, registerContext, createEffectScope } from 'gonia';
import { exists, readDir } from '@tauri-apps/plugin-fs';
import { homeDir } from '@tauri-apps/api/path';
import { SteamIdKey, EditorKey, type EditorState } from '../contexts.js';

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
}

function classDisplayName(raw: string | null) {
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

  registerContext($element, SteamIdKey, steamId);
  registerContext($element, EditorKey, editor);

  Object.assign($scope, {
    editor,
    steamId,
    classDisplayName,
  });

  detectSteamId(steamId);

  return () => effectScope.stop();
}

directive('app-root', AppRootDirective, { scope: true });

const SAVE_BASES = [
  '.local/share/Steam/steamapps/compatdata/1285190/pfx/drive_c/users/steamuser/Documents/My Games/Borderlands 4/Saved/SaveGames',
  'Documents/My Games/Borderlands 4/Saved/SaveGames',
];

async function detectSteamId(steamId: { value: string }) {
  try {
    const home = await homeDir();
    for (const base of SAVE_BASES) {
      const savePath = home.endsWith('/') ? `${home}${base}` : `${home}/${base}`;
      if (!await exists(savePath)) continue;

      const entries = await readDir(savePath);
      for (const entry of entries) {
        if (entry.name && /^\d{17}$/.test(entry.name)) {
          steamId.value = entry.name;
          return;
        }
      }
    }
  } catch {
    // Detection is best-effort
  }
}

export function extractSteamIdFromPath(path: string): string | null {
  const match = path.match(/(\d{17})/);
  return match ? match[1] : null;
}
