import { directive, type Directive } from 'gonia';

interface AppState {
  title: string;
}

const appRoot: Directive<['$element', '$scope']> = (_$element, $scope: AppState) => {
  $scope.title = 'BL4 Save Editor';
};

directive('app-root', appRoot, { scope: true });
