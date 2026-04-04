import init, { decryptSav, encryptSav, SaveFile, ChangeSet } from './wasm/bl4.js';

let initPromise: Promise<void> | null = null;

export async function initBl4(): Promise<void> {
  if (!initPromise) {
    initPromise = init();
  }
  await initPromise;
}

export { decryptSav, encryptSav, SaveFile, ChangeSet };
