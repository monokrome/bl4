import { describe, it, expect, vi } from 'vitest';
import { withSaveField } from './with-save-field.js';

describe('withSaveField', () => {
  describe('template wrapping', () => {
    it('wraps a string template with a save-field-binding element', () => {
      const wrapped = withSaveField(
        { path: 'state.currencies.cash', as: 'cash' },
        '<input />',
      );

      expect(typeof wrapped).toBe('function');
      const rendered = (wrapped as Function)({}, null);

      expect(rendered).toContain('<save-field-binding');
      expect(rendered).toContain('path="state.currencies.cash"');
      expect(rendered).toContain('as="cash"');
      expect(rendered).toContain('<input />');
      expect(rendered).toContain('</save-field-binding>');
    });

    it('wraps a function template by invoking it with attrs+el', () => {
      const userTemplate = vi.fn(() => '<span>inner</span>');

      const wrapped = withSaveField(
        { path: 'state.char_name', as: 'name' },
        userTemplate,
      );

      const attrs = { foo: 'bar' } as any;
      const el = {} as Element;
      const rendered = (wrapped as Function)(attrs, el);

      expect(userTemplate).toHaveBeenCalledWith(attrs, el);
      expect(rendered).toContain('<span>inner</span>');
      expect(rendered).toContain('path="state.char_name"');
      expect(rendered).toContain('as="name"');
    });

    it('handles missing template by wrapping an empty string', () => {
      const wrapped = withSaveField(
        { path: 'a.b.c', as: 'field' },
        undefined,
      );
      const rendered = (wrapped as Function)({}, null);
      expect(rendered).toBe('<save-field-binding path="a.b.c" as="field"></save-field-binding>');
    });
  });

  describe('profile flag', () => {
    it('adds profile="true" attribute when cfg.profile is true', () => {
      const wrapped = withSaveField(
        { path: 'inventory.items.bank', as: 'bank', profile: true },
        '<div />',
      );
      const rendered = (wrapped as Function)({}, null);
      expect(rendered).toContain('profile="true"');
    });

    it('omits the profile attribute when cfg.profile is false or undefined', () => {
      const wrapped = withSaveField(
        { path: 'anything', as: 'x' },
        '<div />',
      );
      const rendered = (wrapped as Function)({}, null);
      expect(rendered).not.toContain('profile=');
    });
  });

  describe('attribute escaping', () => {
    it('escapes quotes in path and as', () => {
      const wrapped = withSaveField(
        { path: 'evil."path"', as: 'name&stuff' },
        '<div />',
      );
      const rendered = (wrapped as Function)({}, null);

      expect(rendered).toContain('path="evil.&quot;path&quot;"');
      expect(rendered).toContain('as="name&amp;stuff"');
    });
  });
});
