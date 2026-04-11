import type { DirectiveOptions } from 'gonia';

/// Escape a value for use in an HTML attribute inside a generated template.
function escapeAttr(v: string): string {
  return v.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
}

/// Configuration for a save-bound field HoC application.
///
/// `path` is the YAML path in the save file (e.g. `state.currencies.cash`).
/// `as` is the scope key the binding will be exposed under — templates
/// inside the wrapped directive read `{as}.value`, `{as}.dirty`, and call
/// `{as}.onChange(next)`.
///
/// `profile: true` binds against the profile session instead of the
/// active character save.
export interface WithSaveFieldOptions {
  path: string;
  as: string;
  profile?: boolean;
}

/// HoC that wraps a directive's template so that, at render time, the
/// user's template is nested inside a `<save-field-binding>` element.
/// The binding directive reads the `path` attribute, resolves the
/// active session (or profile), and exposes a reactive binding at
/// `scope[as]` that the user's template consumes.
///
/// Does NOT wrap the directive function. Pure options transform.
export function withSaveField(
  cfg: WithSaveFieldOptions,
  options: DirectiveOptions = {},
): DirectiveOptions {
  const userTemplate = options.template;
  const pathAttr = escapeAttr(cfg.path);
  const asAttr = escapeAttr(cfg.as);
  const profileAttr = cfg.profile ? ' profile="true"' : '';

  return {
    ...options,
    template: (attrs, el) => {
      const inner =
        typeof userTemplate === 'function'
          ? userTemplate(attrs, el)
          : (userTemplate ?? '');
      const resolved = typeof inner === 'string' ? inner : '';
      return `<save-field-binding path="${pathAttr}" as="${asAttr}"${profileAttr}>${resolved}</save-field-binding>`;
    },
  };
}
