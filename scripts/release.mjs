import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, basename } from 'node:path'
import { execFileSync } from 'node:child_process'
const { version } = JSON.parse(readFileSync('package.json', 'utf8'))
const tag = `v${version}`
function walk(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name)
    return statSync(p).isDirectory() ? walk(p) : [p]
  })
}
const files = walk('release-input').filter((p) => /\.(dmg|exe|msi|deb|rpm|AppImage)$/.test(p))
for (const ext of ['dmg', 'exe', 'msi', 'deb', 'rpm', 'AppImage'])
  if (!files.some((p) => p.endsWith('.' + ext))) throw new Error(`Missing ${ext} installer`)
if (files.filter((p) => p.endsWith('.dmg')).length < 2)
  throw new Error('Missing macOS architecture')
if (files.some((p) => statSync(p).size < 100000))
  throw new Error('An installer is unexpectedly small')
const names = files.map((p) => basename(p))
if (new Set(names).size !== names.length) throw new Error('Duplicate installer names')
const gh = (...args) => execFileSync('gh', args, { stdio: 'inherit' })
let existing
try {
  existing = JSON.parse(
    execFileSync('gh', ['release', 'view', tag, '--json', 'isDraft'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }),
  )
} catch {
  /* The release does not exist yet. */
}
if (existing && !existing.isDraft) {
  console.log(`${tag} is already published; bump version for another release.`)
} else {
  if (!existing)
    gh(
      'release',
      'create',
      tag,
      '--target',
      process.env.GITHUB_SHA,
      '--title',
      `Fiex ${version}`,
      '--notes-file',
      'docs/RELEASE.md',
      '--draft',
    )
  gh('release', 'upload', tag, '--clobber', ...files)
  gh('release', 'edit', tag, '--draft=false', '--latest')
}
