import { defineConfig, devices } from '@playwright/test';

/**
 * Runs the three routes against `vite dev` with no Tauri behind them. The
 * bridge no-ops without a host, so every route has to render on its own.
 * Native panel behaviour is not covered here; it lives in the manual
 * checklist of tech.md section 15.
 */
export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? 'list' : 'html',
  use: {
    baseURL: 'http://127.0.0.1:1420',
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm dev',
    url: 'http://127.0.0.1:1420/island/',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
