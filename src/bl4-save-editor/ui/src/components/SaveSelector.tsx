import { useState } from 'react';

interface Props {
  steamId: string;
  onSteamIdChange: (id: string) => void;
  onSelectFile: () => void;
  onOpenPath: (path: string) => void;
  loading: boolean;
}

export function SaveSelector({ steamId, onSteamIdChange, onSelectFile, onOpenPath, loading }: Props) {
  const [manualPath, setManualPath] = useState('');
  const isTauri = '__TAURI__' in window;

  return (
    <div className="save-selector">
      <h2>Open Save File</h2>

      <div className="form-group">
        <label htmlFor="steam-id">Steam ID</label>
        <input
          id="steam-id"
          type="text"
          value={steamId}
          onChange={(e) => onSteamIdChange(e.target.value)}
          placeholder="e.g., 76561197960521364"
        />
        <small>Your 17-digit Steam ID (find it in your save folder path)</small>
      </div>

      {isTauri ? (
        <button onClick={onSelectFile} disabled={loading || !steamId}>
          {loading ? 'Opening...' : 'Browse for Save File'}
        </button>
      ) : (
        <div className="form-group">
          <label htmlFor="save-path">Save File Path</label>
          <input
            id="save-path"
            type="text"
            value={manualPath}
            onChange={(e) => setManualPath(e.target.value)}
            placeholder="/path/to/1.sav"
          />
          <button
            onClick={() => onOpenPath(manualPath)}
            disabled={loading || !steamId || !manualPath}
          >
            {loading ? 'Opening...' : 'Open'}
          </button>
        </div>
      )}

      <div className="info-box">
        <h3>Save File Locations</h3>
        <p><strong>Windows:</strong></p>
        <code>%LOCALAPPDATA%\Gearbox\Borderlands4\Saved\SaveGames\[SteamID]\</code>
        <p><strong>Linux (Steam/Proton):</strong></p>
        <code>~/.local/share/Steam/steamapps/compatdata/[AppID]/pfx/drive_c/users/steamuser/AppData/Local/Gearbox/Borderlands4/Saved/SaveGames/[SteamID]/</code>
      </div>
    </div>
  );
}
