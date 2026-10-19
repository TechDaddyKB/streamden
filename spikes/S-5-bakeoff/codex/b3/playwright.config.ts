import { defineConfig } from '@playwright/test';
export default defineConfig({ testDir: './e2e', use: { baseURL: 'http://127.0.0.1:5173', viewport: { width: 1280, height: 900 }, colorScheme: 'dark' }, webServer: { command: 'pnpm dev', url: 'http://127.0.0.1:5173', reuseExistingServer: false }, workers: 1 });
