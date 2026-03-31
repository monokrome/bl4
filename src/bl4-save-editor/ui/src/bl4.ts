import init, { decryptSav, encryptSav, SaveFile, ChangeSet } from './wasm/bl4.js';

let initialized = false;

export async function initBl4(): Promise<void> {
  if (initialized) return;
  await init();
  initialized = true;
}

export { decryptSav, encryptSav, SaveFile, ChangeSet };
