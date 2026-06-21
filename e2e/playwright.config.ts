import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  retries: 1,
  fullyParallel: false,
  reporter: [['html', { open: 'never' }], ['list']],
  globalSetup: './global-setup.ts',
  use: {
    baseURL: 'http://localhost:3005',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'infrastructure',
      use: {
        browserName: 'chromium',
        launchOptions: {
          args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
          headless: true,
        },
      },
    },
  ],
});
