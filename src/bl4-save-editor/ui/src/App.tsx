import { useState, useEffect, useCallback } from 'react';
import {
  SaveInfo,
  CharacterInfo,
  InventoryItem,
  openSave,
  saveChanges,
  getSaveInfo,
  getCharacter,
  setCharacter,
  getInventory,
  selectFile,
} from './api';
import { SaveSelector } from './components/SaveSelector';
import { CharacterPanel } from './components/CharacterPanel';
import { InventoryPanel } from './components/InventoryPanel';

type Tab = 'character' | 'inventory' | 'bank';

export default function App() {
  const [saveInfo, setSaveInfo] = useState<SaveInfo | null>(null);
  const [character, setCharacterState] = useState<CharacterInfo | null>(null);
  const [inventory, setInventory] = useState<InventoryItem[]>([]);
  const [activeTab, setActiveTab] = useState<Tab>('character');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [steamId, setSteamId] = useState('');

  const loadSaveData = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const info = await getSaveInfo();
      setSaveInfo(info);
      if (info) {
        const char = await getCharacter();
        setCharacterState(char);
        const inv = await getInventory();
        setInventory(inv);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load save data');
    } finally {
      setLoading(false);
    }
  }, []);

  const handleOpenSave = async (path: string) => {
    if (!steamId) {
      setError('Please enter your Steam ID');
      return;
    }
    try {
      setLoading(true);
      setError(null);
      await openSave(path, steamId);
      await loadSaveData();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to open save');
    } finally {
      setLoading(false);
    }
  };

  const handleSelectFile = async () => {
    const path = await selectFile([{ name: 'Save Files', extensions: ['sav'] }]);
    if (path) {
      handleOpenSave(path);
    }
  };

  const handleSave = async () => {
    try {
      setLoading(true);
      setError(null);
      await saveChanges();
      await loadSaveData();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save');
    } finally {
      setLoading(false);
    }
  };

  const handleCharacterUpdate = async (updates: Partial<CharacterInfo>) => {
    try {
      setError(null);
      await setCharacter({
        name: updates.name ?? undefined,
        cash: updates.cash ?? undefined,
        eridium: updates.eridium ?? undefined,
        xp: updates.xp ?? undefined,
        specialization_xp: updates.specialization_xp ?? undefined,
      });
      await loadSaveData();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to update character');
    }
  };

  useEffect(() => {
    loadSaveData();
  }, [loadSaveData]);

  return (
    <div className="app">
      <header className="header">
        <h1>BL4 Save Editor</h1>
        {saveInfo?.modified && <span className="modified-badge">Modified</span>}
      </header>

      {error && (
        <div className="error-banner">
          {error}
          <button onClick={() => setError(null)}>×</button>
        </div>
      )}

      {!saveInfo ? (
        <SaveSelector
          steamId={steamId}
          onSteamIdChange={setSteamId}
          onSelectFile={handleSelectFile}
          onOpenPath={handleOpenSave}
          loading={loading}
        />
      ) : (
        <>
          <div className="toolbar">
            <span className="save-path">{saveInfo.path}</span>
            <button onClick={handleSave} disabled={loading || !saveInfo.modified}>
              Save Changes
            </button>
            <button onClick={() => setSaveInfo(null)}>Close</button>
          </div>

          <nav className="tabs">
            <button
              className={activeTab === 'character' ? 'active' : ''}
              onClick={() => setActiveTab('character')}
            >
              Character
            </button>
            <button
              className={activeTab === 'inventory' ? 'active' : ''}
              onClick={() => setActiveTab('inventory')}
            >
              Inventory
            </button>
            <button
              className={activeTab === 'bank' ? 'active' : ''}
              onClick={() => setActiveTab('bank')}
            >
              Bank
            </button>
          </nav>

          <main className="content">
            {loading && <div className="loading">Loading...</div>}
            {!loading && activeTab === 'character' && character && (
              <CharacterPanel character={character} onUpdate={handleCharacterUpdate} />
            )}
            {!loading && activeTab === 'inventory' && (
              <InventoryPanel items={inventory} />
            )}
            {!loading && activeTab === 'bank' && (
              <div className="panel">
                <p>Bank functionality coming soon...</p>
              </div>
            )}
          </main>
        </>
      )}
    </div>
  );
}
