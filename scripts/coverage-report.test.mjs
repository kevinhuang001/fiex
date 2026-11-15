import assert from 'node:assert/strict'
import test from 'node:test'
import { createCoverageTrend } from './coverage-report.mjs'

const baseline = {
  generatedAt: '2026-11-15',
  revision: 'initial',
  total: {
    statements: { pct: 50 },
    branches: { pct: 25 },
    functions: { pct: 75 },
    lines: { pct: 50 },
  },
}

test('reports coverage changes in percentage points without gating', () => {
  const summary = {
    total: {
      statements: { covered: 6, total: 10, pct: 60 },
      branches: { covered: 2, total: 4, pct: 50 },
      functions: { covered: 3, total: 4, pct: 75 },
      lines: { covered: 6, total: 10, pct: 60 },
    },
    'src/a.ts': {},
    'src/b.ts': {},
  }
  const report = createCoverageTrend(summary, baseline, new Date('2026-11-16T00:00:00Z'))

  assert.match(report, /Measured source files: 2/)
  assert.match(report, /statements \| 6\/10 \(60\.00%\) \| 50\.00% \| \+10\.00 pp/)
  assert.match(report, /branches \| 2\/4 \(50\.00%\) \| 25\.00% \| \+25\.00 pp/)
  assert.match(report, /This report is informational/)
})

test('rejects incomplete coverage summaries', () => {
  assert.throws(
    () => createCoverageTrend({ total: { statements: { pct: 10 } } }, baseline),
    /Missing branches coverage metric/,
  )
})
