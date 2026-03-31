import { defineConfig } from 'vite';
import { gonia } from 'gonia/vite';

export default defineConfig({
  plugins: [gonia()],
  optimizeDeps: {
    exclude: ['./src/wasm/bl4.js'],
  },
});
