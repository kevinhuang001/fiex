import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  use: {
    baseURL: 'http://127.0.0.1:1420',
    viewport: { width: 1200, height: 780 },
    trace: 'retain-on-failure',
    launchOptions: process.env.FIEX_TEST_CHROMIUM
      ? {
          executablePath: process.env.FIEX_TEST_CHROMIUM,
          args: ['--no-sandbox', '--disable-dev-shm-usage'],
        }
      : {},
  },
  webServer: {
    command: process.env.FIEX_E2E_PREVIEW
      ? 'npx vite preview --host 127.0.0.1 --port 1420'
      : 'npm run dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
  },
  reporter: [['list'], ['html', { open: 'never' }]],
})
