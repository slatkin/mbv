/// <reference types="node" />
import { fileURLToPath, URL } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

// The build output is Tauri's frontendDist (crates/mbv-gui/tauri.conf.json).
export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  resolve: {
    alias: {
      $lib: fileURLToPath(new URL('./src/lib', import.meta.url)),
    },
    // Svelte resolves its browser build under vitest only with this condition.
    conditions: process.env.VITEST ? ['browser'] : undefined,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    setupFiles: ['@testing-library/svelte/vitest', 'src/vitest.setup.ts'],
  },
});
