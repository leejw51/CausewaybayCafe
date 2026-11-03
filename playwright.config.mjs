import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/e2e', timeout: 90000, expect: { timeout: 10000 }, workers: 1,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: { actionTimeout: 12000, baseURL: 'http://127.0.0.1:18890', browserName: 'chromium', viewport: { width: 1440, height: 1000 }, trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: { command: 'python3 scripts/e2e_server.py', url: 'http://127.0.0.1:18890/health', reuseExistingServer: false, timeout: 30000 }
});
