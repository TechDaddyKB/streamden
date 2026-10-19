import { defineConfig } from 'vitest/config';
export default defineConfig({ server: { host: '127.0.0.1', strictPort: true }, test: { environment: 'jsdom', setupFiles: './src/setup.ts', include: ['src/**/*.test.tsx'] } });
