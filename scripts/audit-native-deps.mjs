// Fails release builds that link libraries from the build machine instead of
// shipping self-contained code. Homebrew libraries are the classic offender:
// they are visible at build time but rejected by dyld (different Team ID) once
// the app is ad-hoc signed on Apple Silicon.
import { execFileSync } from 'node:child_process'
import { existsSync, readdirSync, statSync } from 'node:fs'
import { basename, dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const targetIndex = process.argv.indexOf('--target')
const target = targetIndex >= 0 ? process.argv[targetIndex + 1] : process.env.FIEX_TARGET
const release = join(root, 'src-tauri', 'target', ...(target ? [target] : []), 'release')
const appBundle = join(release, 'bundle', 'macos', 'Fiex.app')
const binary =
  process.env.FIEX_AUDIT_BINARY ||
  (process.platform === 'darwin'
    ? join(appBundle, 'Contents', 'MacOS', 'fiex')
    : process.platform === 'win32'
      ? join(release, 'fiex.exe')
      : join(release, 'fiex'))

if (!existsSync(binary)) {
  console.error(`No native binary found at ${binary}; build the desktop app first.`)
  process.exit(1)
}

const suspicious = /\/(opt\/homebrew|usr\/local|Users|home|Cellar|private\/tmp|Volumes)\//
const offenders = []
const relative = (file) => file.replace(root + '/', '')

if (process.platform === 'darwin') {
  const inspect = (file) => {
    const output = execFileSync('otool', ['-L', file], { encoding: 'utf8' })
    for (const line of output.split('\n').slice(1)) {
      const path = line.trim().split(' (compatibility')[0]
      if (path && suspicious.test(path)) offenders.push(`${relative(file)}: ${path}`)
    }
  }
  inspect(binary)
  const commands = execFileSync('otool', ['-l', binary], { encoding: 'utf8' })
  for (const line of commands.split('\n')) {
    const match = line.trim().match(/^path (.+) \(offset \d+\)$/)
    if (match && suspicious.test(match[1])) offenders.push(`${relative(binary)} rpath: ${match[1]}`)
  }
  if (existsSync(appBundle)) {
    const walk = (dir) => {
      for (const name of readdirSync(dir)) {
        const path = join(dir, name)
        if (statSync(path).isDirectory()) walk(path)
        else if (path.endsWith('.dylib') || path.endsWith('.so')) inspect(path)
      }
    }
    walk(appBundle)
  }
} else if (process.platform === 'linux') {
  const output = execFileSync('ldd', [binary], { encoding: 'utf8' })
  for (const line of output.split('\n')) {
    const path = (line.match(/=>\s+(\/\S+)/) || line.match(/^\s*(\/\S+)/))?.[1]
    if (path && suspicious.test(path)) offenders.push(`${basename(binary)}: ${path}`)
  }
  try {
    const dynamic = execFileSync('readelf', ['-d', binary], { encoding: 'utf8' })
    for (const line of dynamic.split('\n')) {
      const match = line.match(/(?:RPATH|RUNPATH).*\[([^\]]+)\]/)
      if (match)
        for (const entry of match[1].split(':'))
          if (suspicious.test(entry)) offenders.push(`${basename(binary)} rpath: ${entry}`)
    }
  } catch {
    // readelf is optional; ldd already covers the linked libraries.
  }
} else {
  console.log('Native dependency audit is not implemented on this platform; skipping.')
  process.exit(0)
}

if (offenders.length) {
  console.error('Release binary depends on libraries from the build machine:')
  for (const line of [...new Set(offenders)]) console.error(`  ${line}`)
  console.error('Link these dependencies statically before publishing.')
  process.exit(1)
}
console.log(`Self-contained native dependency check passed for ${binary}.`)
