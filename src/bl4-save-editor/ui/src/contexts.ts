import { createContextKey, resolveContext } from 'gonia';

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

export const SteamIdKey = createContextKey<{ value: string }>('steamId');
export const EditorKey = createContextKey<EditorState>('editor');

export function requireContext<T>(element: Element, key: ReturnType<typeof createContextKey<T>>): T {
  const ctx = resolveContext(element, key);
  if (ctx === undefined) {
    throw new Error(`Missing context provider for "${String(key)}". Ensure a parent element registers this context.`);
  }
  return ctx;
}
