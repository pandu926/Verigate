import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  retries: 0,
  fullyParallel: false,
  reporter: [['list']],
  use: {
    baseURL: process.env.BASE_URL || 'http://verigate.rbexp.com',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'phase1',
      testMatch: '*.spec.ts',
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
