import type { DirectiveOptions } from 'gonia';

/// Escape a value for use in an HTML attribute inside a generated template.
function escapeAttr(v: string): string {
  return v.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
}

/// Configuration for a save-bound field template wrapper.
///
/// `path` is the YAML path in the save file (e.g. `state.currencies.cash`).
/// `as` is the scope key the binding will be exposed under — templates
/// inside the wrapped content read `{as}.value`, `{as}.dirty`, and call
/// `{as}.onChange(next)`.
///
/// `profile: true` binds against the profile session instead of the
/// active character save.
export interface SaveFieldConfig {
  path: string;
  as: string;
  profile?: boolean;
}

/// Wrap a template (string or function) so that, at render time, the
/// content is nested inside a `<save-field-binding>` element that
/// exposes a reactive binding on its scope. The returned value is
/// suitable as a drop-in for the `template` field on DirectiveOptions.
///
/// This is a template helper, not an options HoC — the directive call
/// stays a literal options object so bellagonia's static transform
/// can inject `$styles` without breakage.
export function withSaveField(
  cfg: SaveFieldConfig,
  template: DirectiveOptions['template'],
): DirectiveOptions['template'] {
  const pathAttr = escapeAttr(cfg.path);
  const asAttr = escapeAttr(cfg.as);
  const profileAttr = cfg.profile ? ' profile="true"' : '';

  return (attrs, el) => {
    const inner = typeof template === 'function' ? template(attrs, el) : (template ?? '');
    const resolved = typeof inner === 'string' ? inner : '';
    return `<save-field-binding path="${pathAttr}" as="${asAttr}"${profileAttr}>${resolved}</save-field-binding>`;
  };
}
