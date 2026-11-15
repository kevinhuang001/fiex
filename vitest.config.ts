import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    alias: { '@': new URL('./src', import.meta.url).pathname },
  },
  test: {
    include: ['src/**/*.test.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.test.{ts,tsx}', 'src/**/*.d.ts'],
      all: true,
      reporter: ['text', 'json-summary', 'lcov'],
      reportsDirectory: 'coverage',
      reportOnFailure: true,
    },
  },
})
