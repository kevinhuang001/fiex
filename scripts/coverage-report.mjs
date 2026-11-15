import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const root = process.cwd()
const summaryPath = resolve(root, 'coverage/frontend/coverage-summary.json')
const baselinePath = resolve(root, 'scripts/coverage-baseline.json')
const outputPath = resolve(root, 'coverage/frontend/coverage-trend.md')

export function createCoverageTrend(summary, baseline, generatedAt = new Date()) {
  const total = summary.total
  if (!total || !baseline.total) {
    throw new Error('Coverage summary or baseline has no total metrics')
  }

  const metricNames = ['statements', 'branches', 'functions', 'lines']
  const rows = metricNames.map((name) => {
    const current = total[name]
    const initial = baseline.total[name]
    if (!current || !initial) throw new Error(`Missing ${name} coverage metric`)
    const delta = Number((current.pct - initial.pct).toFixed(2))
    const change = `${delta >= 0 ? '+' : ''}${delta.toFixed(2)} pp`
    return `| ${name} | ${current.covered}/${current.total} (${current.pct.toFixed(2)}%) | ${initial.pct.toFixed(2)}% | ${change} |`
  })
  const sourceFiles = Object.keys(summary).filter((key) => key !== 'total').length
  return [
    '# Frontend coverage trend',
    '',
    `Generated: ${generatedAt.toISOString()}`,
    `Baseline: ${baseline.generatedAt} (${baseline.revision})`,
    `Measured source files: ${sourceFiles}`,
    '',
    'This report is informational. It does not impose a coverage threshold.',
    '',
    '| Metric | Current | Baseline | Change |',
    '| --- | ---: | ---: | ---: |',
    ...rows,
    '',
  ].join('\n')
}

function main() {
  if (!existsSync(summaryPath)) {
    process.stderr.write('Coverage summary is missing; the coverage run may not have completed.\n')
    return
  }
  const summary = JSON.parse(readFileSync(summaryPath, 'utf8'))
  const baseline = JSON.parse(readFileSync(baselinePath, 'utf8'))
  const report = createCoverageTrend(summary, baseline)

  mkdirSync(resolve(root, 'coverage/frontend'), { recursive: true })
  writeFileSync(outputPath, report)
  process.stdout.write(report)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main()
}
