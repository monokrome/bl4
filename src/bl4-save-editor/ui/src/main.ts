import './global.css.js';
import './directives/app-root.js';
import './directives/app-header.js';
import './directives/file-open.js';
import './directives/save-list.js';
import './directives/editor-view.js';
import './directives/save-field-binding/index.js';
import './directives/panel/index.js';
import './directives/panel-grid/index.js';
import './directives/labeled-field/index.js';
import './directives/dirty-indicator/index.js';
import './directives/character-cash/index.js';
import { hydrate } from 'gonia/client';

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => hydrate());
} else {
  hydrate();
}
