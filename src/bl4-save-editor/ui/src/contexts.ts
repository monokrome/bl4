import { createContextKey } from 'gonia';

export interface SaveInfo {
  path: string;
  name: string;
  isProfile: boolean;
  characterName: string | null;
  characterClass: string | null;
  level: number | null;
  difficulty: string | null;
  hint: string;
}

export interface EditorState {
  saves: SaveInfo[];
  activeSave: string | null;
  activeSection: string | null;
  drawerOpen: boolean;
  loading: boolean;
  error: string | null;
}

export const SteamIdContext = createContextKey<{ value: string }>('steamId');
export const EditorContext = createContextKey<EditorState>('editor');
