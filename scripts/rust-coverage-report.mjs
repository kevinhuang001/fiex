import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const root = process.cwd()
const summaryPath = resolve(root, 'coverage/rust/rust-coverage.json')
const baselinePath = resolve(root, 'scripts/rust-coverage-baseline.json')
const outputPath = resolve(root, 'coverage/rust/rust-coverage-trend.md')
const metricNames = ['lines', 'functions', 'regions']

function aggregateCoverage(summary) {
  if (!Array.isArray(summary.data) || summary.data.length === 0) {
    throw new Error('Rust coverage summary has no data records')
  }
  const files = summary.data.flatMap((record) => record.files ?? [])
  const total = Object.fromEntries(
    metricNames.map((name) => {
      const values = summary.data.map((record) => record.totals?.[name])
      if (values.some((value) => !value)) throw new Error(`Missing Rust ${name} metric`)
      const count = values.reduce((sum, value) => sum + value.count, 0)
      const covered = values.reduce((sum, value) => sum + value.covered, 0)
      return [name, { count, covered, percent: count === 0 ? 100 : (covered / count) * 100 }]
    }),
  )
  return { sourceFiles: files.length, total }
}

export function createRustCoverageTrend(summary, baseline, generatedAt = new Date()) {
  if (!baseline.total) throw new Error('Rust coverage baseline has no total metrics')
  const current = aggregateCoverage(summary)
  const rows = metricNames.map((name) => {
    const actual = current.total[name]
    const initial = baseline.total[name]
    if (!initial) throw new Error(`Missing baseline Rust ${name} metric`)
    const delta = Number((actual.percent - initial.percent).toFixed(2))
    const change = `${delta >= 0 ? '+' : ''}${delta.toFixed(2)} pp`
    return `| ${name} | ${actual.covered}/${actual.count} (${actual.percent.toFixed(2)}%) | ${initial.percent.toFixed(2)}% | ${change} |`
  })
  return [
    '# Rust workspace coverage trend',
    '',
    `Generated: ${generatedAt.toISOString()}`,
    `Baseline: ${baseline.generatedAt} (${baseline.revision})`,
    `Measured source files: ${current.sourceFiles}`,
    '',
    'This report is informational. It does not impose a coverage threshold.',
    'Branch and MC/DC coverage are not reported by this stable LLVM coverage profile.',
    '',
    '| Metric | Current | Baseline | Change |',
    '| --- | ---: | ---: | ---: |',
    ...rows,
    '',
  ].join('\n')
}

function main() {
  if (!existsSync(summaryPath)) {
    process.stderr.write(
      'Rust coverage summary is missing; the measurement may not have completed.\n',
    )
    return
  }
  const summary = JSON.parse(readFileSync(summaryPath, 'utf8'))
  const baseline = JSON.parse(readFileSync(baselinePath, 'utf8'))
  const report = createRustCoverageTrend(summary, baseline)
  mkdirSync(resolve(root, 'coverage/rust'), { recursive: true })
  writeFileSync(outputPath, report)
  process.stdout.write(report)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main()
}
