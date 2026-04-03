import './global.css.js';
import './directives/app-root.js';
import './directives/app-header.js';
import './directives/file-open.js';
import './directives/save-list.js';
import './directives/editor-view.js';
import { hydrate } from 'gonia/client';

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => hydrate());
} else {
  hydrate();
}
