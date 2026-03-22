import { defineConfig } from 'vitest/config';
import { resolve } from 'path';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));

export default defineConfig({
  resolve: {
    alias: {
      '@racingpoint/types': resolve(__dirname, '../shared-types/src/index.ts'),
    },
  },
  test: {
    globals: true,
    environment: 'node',
  },
});
