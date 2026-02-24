import { directive } from 'gonia'
import * as api from '../api/invoke.js'

directive('save-selector', function saveSelector(
  $element: Element,
  $scope: Record<string, unknown>,
) {
  $scope['steamId'] = ''
  $scope['selectedPath'] = ''

  $scope['browseFile'] = async () => {
    const path = await api.openFileDialog()
    if (path) {
      $scope['selectedPath'] = path as string
    }
  }

  $scope['openSelected'] = () => {
    const path = $scope['selectedPath'] as string
    const steamId = $scope['steamId'] as string
    if (!path) return
    const loadSave = $scope['loadSave'] as (path: string, steamId: string) => void
    if (loadSave) {
      loadSave(path, steamId)
    }
  }

  $element.innerHTML = `
    <div class="save-selector">
      <h2>Open Save File</h2>
      <div class="panel">
        <div class="panel-section">
          <div class="form-field">
            <label>Steam ID</label>
            <input g-model="steamId" type="text" placeholder="Enter Steam ID for decryption" />
            <div class="form-hint">Required to decrypt .sav files</div>
          </div>
          <div class="form-field">
            <label>Save File</label>
            <div style="display: flex; gap: 0.5rem">
              <input g-model="selectedPath" type="text" placeholder="No file selected" readonly style="flex: 1" />
              <button class="btn btn-primary" g-on="click: browseFile">Browse</button>
            </div>
          </div>
        </div>
        <div style="text-align: right; margin-top: 1rem">
          <button class="btn btn-primary" g-on="click: openSelected">Open</button>
        </div>
      </div>
      <div class="info-box">
        <h3>Save Location</h3>
        <p>Windows:</p>
        <code>%LOCALAPPDATA%/Gearbox Software/Borderlands 4/Saved/SaveGames/&lt;SteamID&gt;/</code>
        <p>Linux (Proton):</p>
        <code>~/.local/share/Steam/steamapps/compatdata/.../pfx/drive_c/...</code>
      </div>
    </div>
  `
}, { scope: true })
