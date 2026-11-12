import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { otoolsTauriShimPlugin } from 'otools-plugin-sdk/vite';
import { fileURLToPath } from 'node:url';

export default defineConfig({
  base: './',
  plugins: [vue(), otoolsTauriShimPlugin()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  build: { outDir: '../dist', emptyOutDir: true },
  server: { port: 6546, strictPort: true },
});
