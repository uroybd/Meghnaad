import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  // Tests mount components, which needs Svelte's browser build rather than its server one.
  resolve: process.env.VITEST ? { conditions: ['browser'] } : {},
  server: {
    // `npm run dev` here, `npm run dev` (wrangler) at the repo root for the API.
    proxy: { '/api': process.env.API_URL ?? 'http://127.0.0.1:8787' },
  },
  build: { outDir: 'dist', sourcemap: false, target: 'es2022' },
  test: { environment: 'jsdom', include: ['src/**/*.test.ts'] },
});
