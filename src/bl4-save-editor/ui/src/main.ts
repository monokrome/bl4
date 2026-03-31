import './directives/app-root.js';
import { hydrate } from 'gonia/client';
import { initBl4 } from './bl4.js';

async function start() {
  await initBl4();
  hydrate();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => start());
} else {
  start();
}
