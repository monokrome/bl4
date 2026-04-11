import { directive } from 'gonia';
import template from './template.pug';
import { withSaveField } from '../../hocs/with-save-field.js';
import { EditorContext } from '../../contexts.js';

/// Cash amount on the active character save.
/// The pug template reads/writes `cash.value` and `cash.dirty`, bound
/// via the withSaveField helper. Directive function is null — the
/// template and the save-field-binding child directive do all the work.
directive('character-cash', null, {
  scope: true,
  template: withSaveField({ path: 'state.currencies.cash', as: 'cash' }, template),
  using: [EditorContext],
});
