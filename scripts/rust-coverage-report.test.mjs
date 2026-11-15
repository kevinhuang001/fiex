import assert from 'node:assert/strict'
import test from 'node:test'
import { createRustCoverageTrend } from './rust-coverage-report.mjs'

const baseline = {
  generatedAt: '2026-11-15',
  revision: 'initial',
  total: {
    lines: { count: 20, covered: 10, percent: 50 },
    functions: { count: 10, covered: 5, percent: 50 },
    regions: { count: 30, covered: 15, percent: 50 },
  },
}

test('reports stable Rust coverage metrics against the baseline', () => {
  const summary = {
    data: [
      {
        files: [{ filename: 'src/lib.rs' }],
        totals: {
          lines: { count: 20, covered: 12 },
          functions: { count: 10, covered: 6 },
          regions: { count: 30, covered: 18 },
        },
      },
    ],
  }
  const report = createRustCoverageTrend(summary, baseline, new Date('2026-11-16T00:00:00Z'))

  assert.match(report, /Measured source files: 1/)
  assert.match(report, /lines \| 12\/20 \(60\.00%\) \| 50\.00% \| \+10\.00 pp/)
  assert.match(report, /regions \| 18\/30 \(60\.00%\) \| 50\.00% \| \+10\.00 pp/)
  assert.match(report, /Branch and MC\/DC coverage are not reported/)
})

test('rejects missing workspace coverage records', () => {
  assert.throws(() => createRustCoverageTrend({ data: [] }, baseline), /no data records/)
})

test('rounds sub-hundredth regressions to positive zero', () => {
  const summary = {
    data: [
      {
        files: [],
        totals: {
          lines: { count: 100_000, covered: 49_999 },
          functions: { count: 10, covered: 5 },
          regions: { count: 30, covered: 15 },
        },
      },
    ],
  }
  const initial = {
    ...baseline,
    total: {
      ...baseline.total,
      lines: { count: 100_000, covered: 50_000, percent: 50 },
    },
  }
  const report = createRustCoverageTrend(summary, initial)

  assert.match(report, /lines \| 49999\/100000 \(50\.00%\) \| 50\.00% \| \+0\.00 pp/)
  assert.doesNotMatch(report, /-0\.00 pp/)
})
