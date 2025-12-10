import { defineConfig } from 'vite';

export default defineConfig({
  root: './web',
  build: {
    target: 'esnext',
    outDir: '../dist',
    emptyOutDir: true,
  },
  server: {
    port: 3000,
    open: true,
  },
  optimizeDeps: {
    exclude: ['procedural-terrain-simulator'],
  },
});
