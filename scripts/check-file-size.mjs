#!/usr/bin/env node
/**
 * Keep new modules small. Existing large files are recorded in the baseline so
 * this PR can land without a risky mass rewrite; every refactor should remove
 * entries from the baseline. A baselined file may shrink but never grow.
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const ROOT = process.cwd()
const LIMIT = 300
const BASELINE = join(ROOT, 'scripts', 'file-size-baseline.json')
const roots = ['src', 'src-tauri/src'].map((path) => join(ROOT, path))
const extensions = new Set(['.rs', '.ts', '.tsx'])
const ignored = new Set(['node_modules', 'dist', 'target', 'gen', 'test-results', 'artifacts'])

function walk(directory, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (ignored.has(entry.name)) continue
    const path = join(directory, entry.name)
    if (entry.isDirectory()) walk(path, files)
    else if (extensions.has(path.slice(path.lastIndexOf('.')))) files.push(path)
  }
  return files
}

const baseline = JSON.parse(readFileSync(BASELINE, 'utf8'))
const failures = []
let allowed = 0
for (const file of roots.flatMap((root) => walk(root))) {
  const path = relative(ROOT, file).replaceAll('\\', '/')
  const lines = readFileSync(file, 'utf8').split('\n').length
  if (lines <= LIMIT) continue
  const recorded = baseline[path]
  if (recorded === undefined) {
    failures.push(`${path}: ${lines} lines; split it or add it to scripts/file-size-baseline.json`)
    continue
  }
  allowed += 1
  if (lines > recorded) {
    failures.push(
      `${path}: grew from ${recorded} to ${lines} lines; baselined files may only shrink`,
    )
  }
}
if (failures.length) {
  console.error('File size check failed:')
  for (const failure of failures) console.error(`- ${failure}`)
  process.exit(1)
}
console.log(`File size check passed: ${allowed} baselined legacy files, limit ${LIMIT} lines.`)
