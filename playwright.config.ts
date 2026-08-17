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
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm dev',
    // Matches the devUrl in tauri.conf.json. Polling 127.0.0.1 while vite
    // binds localhost misses it whenever localhost resolves to ::1 first.
    url: 'http://localhost:1420/island/',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
