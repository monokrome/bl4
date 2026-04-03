import { defineConfig } from 'vite';
import { gonia } from 'gonia/vite';
import { bellagonia } from 'bellagonia';

export default defineConfig({
  plugins: [
    gonia(),
    bellagonia({ directiveSources: ['src/directives/**/*.ts'] }),
  ],
  optimizeDeps: {
    exclude: ['./src/wasm/bl4.js'],
  },
});
