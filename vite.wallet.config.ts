import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';

// A separate release build prevents the legacy password/Telegram UI and its
// dependencies from being bundled into the native wallet application.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(__dirname, '.') } },
  server: {
    host: process.env.TAURI_DEV_HOST || '127.0.0.1',
    port: 3000,
    strictPort: true,
  },
  build: {
    outDir: 'dist-wallet',
    emptyOutDir: true,
    rollupOptions: { input: path.resolve(__dirname, 'wallet.html') },
  },
});
