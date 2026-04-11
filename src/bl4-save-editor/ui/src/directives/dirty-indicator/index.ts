import { directive } from 'gonia';
import template from './template.pug';
import { EditorContext, type EditorState } from '../../contexts.js';

/// Visual marker that shows when a path has a pending change.
/// Clicking it opens the changeset drawer and focuses the entry.
///
/// Usage: `<dirty-indicator show="cash.dirty" focus-path="state.currencies.cash" />`
/// Place inside a save-field-binding so it can read the `dirty` flag
/// directly from the bound scope value via an expression attribute.
interface DirtyIndicatorScope {
  show: boolean;
  focusPath: string;
  handleClick: () => void;
}

export function DirtyIndicatorDirective(
  $element: Element,
  $scope: DirtyIndicatorScope,
  editor: EditorState,
) {
  if ($scope.show === undefined) $scope.show = false;
  $scope.focusPath = $element.getAttribute('focus-path') ?? '';

  $scope.handleClick = () => {
    editor.drawerOpen = true;
    document.dispatchEvent(
      new CustomEvent('bl4:focus-dirty', { detail: { path: $scope.focusPath } }),
    );
  };
}
DirtyIndicatorDirective.$inject = ['$element', '$scope'];

directive('dirty-indicator', DirtyIndicatorDirective, {
  scope: true,
  template,
  using: [EditorContext],
});
