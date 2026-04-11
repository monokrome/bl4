import { defineConfig, type Plugin } from 'vite';
import { gonia } from 'gonia/vite';
import { bellagonia } from 'bellagonia';
import { readFileSync } from 'node:fs';
import pug from 'pug';

/// Transform `.pug` imports into compiled HTML string modules.
/// Directive templates write pug syntax and import them like:
///   import template from './template.pug';
/// The file is compiled to HTML at build time.
function pugTransform(): Plugin {
  return {
    name: 'bl4-pug-transform',
    enforce: 'pre',
    load(id) {
      if (!id.endsWith('.pug')) return null;
      const source = readFileSync(id, 'utf-8');
      const html = pug.render(source, { filename: id });
      return `export default ${JSON.stringify(html)};`;
    },
  };
}

export default defineConfig({
  plugins: [
    pugTransform(),
    gonia(),
    bellagonia({ directiveSources: ['src/directives/**/*.ts'] }),
  ],
  optimizeDeps: {
    exclude: ['./src/wasm/bl4.js'],
  },
});
