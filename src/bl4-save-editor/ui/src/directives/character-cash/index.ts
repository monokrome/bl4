import { directive } from 'gonia';
import template from './template.pug';
import { withSaveField } from '../../hocs/with-save-field.js';

/// Cash amount on the active character save.
/// Data binding handled entirely by the withSaveField HoC — the
/// directive function is null.
directive(
  'character-cash',
  null,
  withSaveField({ path: 'state.currencies.cash', as: 'cash' }, {
    scope: true,
    template,
  }),
);
