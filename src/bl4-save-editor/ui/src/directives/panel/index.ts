import { directive } from 'gonia';
import template from './template.pug';

/// Titled surface that groups related content. Renders a header with
/// an optional title and subtitle, then a body slot for children.
interface PanelScope {
  title: string;
  subtitle: string;
}

export function PanelDirective($scope: PanelScope) {
  if ($scope.title === undefined) $scope.title = '';
  if ($scope.subtitle === undefined) $scope.subtitle = '';
}
PanelDirective.$inject = ['$scope'];

directive('panel', PanelDirective, {
  scope: true,
  template,
});
