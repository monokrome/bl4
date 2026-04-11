import { describe, it, expect, vi } from 'vitest';
import { withSaveField } from './with-save-field.js';

describe('withSaveField', () => {
  describe('template wrapping', () => {
    it('wraps a string template with a save-field-binding element', () => {
      const options = withSaveField(
        { path: 'state.currencies.cash', as: 'cash' },
        { scope: true, template: '<input />' },
      );

      expect(typeof options.template).toBe('function');
      const rendered = (options.template as Function)({}, null);

      expect(rendered).toContain('<save-field-binding');
      expect(rendered).toContain('path="state.currencies.cash"');
      expect(rendered).toContain('as="cash"');
      expect(rendered).toContain('<input />');
      expect(rendered).toContain('</save-field-binding>');
    });

    it('wraps a function template by invoking it with attrs+el', () => {
      const userTemplate = vi.fn(() => '<span>inner</span>');

      const options = withSaveField(
        { path: 'state.char_name', as: 'name' },
        { scope: true, template: userTemplate },
      );

      const attrs = { foo: 'bar' } as any;
      const el = {} as Element;
      const rendered = (options.template as Function)(attrs, el);

      expect(userTemplate).toHaveBeenCalledWith(attrs, el);
      expect(rendered).toContain('<span>inner</span>');
      expect(rendered).toContain('path="state.char_name"');
      expect(rendered).toContain('as="name"');
    });

    it('handles missing template by wrapping an empty string', () => {
      const options = withSaveField(
        { path: 'a.b.c', as: 'field' },
        { scope: true },
      );

      const rendered = (options.template as Function)({}, null);
      expect(rendered).toBe('<save-field-binding path="a.b.c" as="field"></save-field-binding>');
    });
  });

  describe('profile flag', () => {
    it('adds profile="true" attribute when cfg.profile is true', () => {
      const options = withSaveField(
        { path: 'inventory.items.bank', as: 'bank', profile: true },
        { scope: true, template: '<div />' },
      );
      const rendered = (options.template as Function)({}, null);
      expect(rendered).toContain('profile="true"');
    });

    it('omits the profile attribute when cfg.profile is false or undefined', () => {
      const options = withSaveField(
        { path: 'anything', as: 'x' },
        { scope: true, template: '<div />' },
      );
      const rendered = (options.template as Function)({}, null);
      expect(rendered).not.toContain('profile=');
    });
  });

  describe('attribute escaping', () => {
    it('escapes quotes in path and as', () => {
      const options = withSaveField(
        { path: 'evil."path"', as: 'name&stuff' },
        { scope: true, template: '<div />' },
      );
      const rendered = (options.template as Function)({}, null);

      expect(rendered).toContain('path="evil.&quot;path&quot;"');
      expect(rendered).toContain('as="name&amp;stuff"');
    });
  });

  describe('options passthrough', () => {
    it('preserves unrelated options like scope and assign', () => {
      const customAssign = { $extra: 'value' };
      const options = withSaveField(
        { path: 'p', as: 'f' },
        { scope: true, assign: customAssign, template: '<div />' },
      );

      expect(options.scope).toBe(true);
      expect(options.assign).toBe(customAssign);
    });

    it('does not mutate the input options object', () => {
      const original = { scope: true, template: '<div />' };
      const options = withSaveField({ path: 'p', as: 'f' }, original);
      expect(original.template).toBe('<div />');
      expect(options.template).not.toBe(original.template);
    });
  });
});
