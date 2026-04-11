import { directive } from 'gonia';
import template from './template.pug';

/// Label + hint above a form control slot.
interface LabeledFieldScope {
  label: string;
  hint: string;
}

export function LabeledFieldDirective($scope: LabeledFieldScope) {
  if ($scope.label === undefined) $scope.label = '';
  if ($scope.hint === undefined) $scope.hint = '';
}
LabeledFieldDirective.$inject = ['$scope'];

directive('labeled-field', LabeledFieldDirective, {
  scope: true,
  template,
});
