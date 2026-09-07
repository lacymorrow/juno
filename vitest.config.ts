import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    // Agent and Paperclip worktrees live inside the repo; their copies of the
    // suite must not run against this tree's React.
    exclude: ['**/node_modules/**', '**/.claude/**', '**/.paperclip/**', '**/target/**'],
  },
});