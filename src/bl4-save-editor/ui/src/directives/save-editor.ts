import { directive, type Directive } from 'gonia';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

interface CharacterInfo {
  name: string | null;
  class: string | null;
  difficulty: string | null;
  level: number | null;
  xp: number | null;
  cash: number | null;
  eridium: number | null;
}

interface SaveEditorState {
  steamId: string;
  loaded: boolean;
  characterName: string;
  info: CharacterInfo;
  targetLevel: number;
  levelResult: string;
  openFile: () => Promise<void>;
  setItemLevel: () => Promise<void>;
}

const saveEditor: Directive<['$element', '$scope']> = (_$element, $scope: SaveEditorState) => {
  $scope.steamId = '';
  $scope.loaded = false;
  $scope.characterName = '';
  $scope.info = {
    name: null,
    class: null,
    difficulty: null,
    level: null,
    xp: null,
    cash: null,
    eridium: null,
  };
  $scope.targetLevel = 60;
  $scope.levelResult = '';

  $scope.openFile = async () => {
    const path = await open({
      filters: [{ name: 'Save Files', extensions: ['sav'] }],
    });
    if (!path) return;

    const name = await invoke<string>('load_save', {
      path,
      steamId: $scope.steamId,
    });

    $scope.characterName = name;
    $scope.info = await invoke<CharacterInfo>('get_character_info');
    $scope.loaded = true;
  };

  $scope.setItemLevel = async () => {
    const count = await invoke<number>('set_item_level', {
      level: $scope.targetLevel,
    });
    $scope.levelResult = `Updated ${count} items to level ${$scope.targetLevel}`;
  };
};

directive('save-editor', saveEditor, { scope: true });
