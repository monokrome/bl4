import { directive } from 'gonia';
import template from './template.pug';

/// Responsive grid container that wraps panels into columns.
export function PanelGridDirective() {
  // Pure layout — no state.
}
PanelGridDirective.$inject = [];

directive('panel-grid', PanelGridDirective, {
  scope: true,
  template,
});
