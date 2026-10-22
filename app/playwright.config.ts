import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: 'e2e',
  use: { baseURL: 'http://localhost:1421' },
  webServer: {
    command: 'npm run dev:mock -- --port 1421 --strictPort',
    url: 'http://localhost:1421',
    reuseExistingServer: !process.env.CI,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
