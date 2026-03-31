import './directives/app-root.js';
import { hydrate } from 'gonia/client';

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => hydrate());
} else {
  hydrate();
}
