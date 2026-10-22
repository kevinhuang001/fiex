/**
 * Compile and stage first-party plugins as the SAME manifest/native process
 * package shape consumed by the generic plugin loader. No special runtime ID
 * branches or Tauri externalBin path are required.
 *
 * Usage: node scripts/package-plugins.mjs [target-triple] [--debug]
 */
import { execFileSync } from 'node:child_process'
import { chmodSync, cpSync, copyFileSync, mkdirSync, rmSync } from 'node:fs'
import { resolve, join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const args = process.argv.slice(2)
const debug = args.includes('--debug')
const supplied = args.find((arg) => !arg.startsWith('--'))
const rustc = execFileSync('rustc', ['-vV'], { encoding: 'utf8' })
const target =
  supplied ||
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  process.env.CARGO_BUILD_TARGET ||
  rustc.match(/^host:\s*(.+)$/m)?.[1]
if (!target || !/^[A-Za-z0-9_-]+$/.test(target))
  throw new Error('Unable to determine a valid Rust target triple')

const profile = debug ? 'debug' : 'release'
const srcDir = join(root, 'plugins', 'official')
const destination = join(root, 'src-tauri', 'plugins')
const suffix = target.includes('windows') ? '.exe' : ''
const packages = [
  { slug: 'webdav', id: 'org.fiex.webdav' },
  { slug: 'smb', id: 'org.fiex.smb' },
  { slug: 'git', id: 'org.fiex.git' },
  { slug: 'archive', id: 'org.fiex.archive' },
  { slug: 'checksum', id: 'org.fiex.checksum' },
]
for (const { slug, id } of packages) {
  const packageDir = join(srcDir, slug)
  console.log('[Fiex plugins]', slug, target, profile)
  execFileSync(
    'cargo',
    [
      'build',
      '--manifest-path',
      join(packageDir, 'Cargo.toml'),
      '--target',
      target,
      '--bin',
      'fiex-plugin-' + slug,
      ...(debug ? [] : ['--release']),
    ],
    { stdio: 'inherit' },
  )
  const src = join(packageDir, 'target', target, profile, 'fiex-plugin-' + slug + suffix)
  const dest = join(destination, id)
  rmSync(dest, { recursive: true, force: true })
  mkdirSync(dest, { recursive: true })
  copyFileSync(join(packageDir, 'manifest.json'), join(dest, 'manifest.json'))
  const executable = join(dest, 'fiex-plugin-' + slug + suffix)
  copyFileSync(src, executable)
  if (process.platform !== 'win32') chmodSync(executable, 0o755)
  // Developers provide compiled iframe assets if the plugin has a UI entry.
  try {
    cpSync(join(packageDir, 'ui', 'dist'), join(dest, 'ui'), { recursive: true })
  } catch (error) {
    if (error.code !== 'ENOENT') throw error
  }
}
console.log('[Fiex plugins] packaged:', packages.map((p) => p.id).join(', '))
