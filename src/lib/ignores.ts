import { inferShortcutStyle } from './shortcuts'

const MAC = ['.fiex', '.DS_Store', '._*', '.Spotlight-V100', '.fseventsd', '.Trashes']
const WINDOWS = ['.fiex', 'Thumbs.db', 'desktop.ini', '$RECYCLE.BIN', 'System Volume Information']
const LINUX = ['.fiex', '.directory', '.Trash-*']

const BUILD = [
  '.git',
  '.svn',
  '.hg',
  '.bzr',
  'node_modules',
  'bower_components',
  'jspm_packages',
  '.yarn',
  '.pnpm-store',
  '.npm',
  'target',
  'build',
  'dist',
  'out',
  '.next',
  '.nuxt',
  '.svelte-kit',
  '.turbo',
  '.parcel-cache',
  '.cache',
  'coverage',
  '__pycache__',
  '.pytest_cache',
  '.mypy_cache',
  '.ruff_cache',
  '.tox',
  '.venv',
  'venv',
  'env',
  'virtualenv',
  '.eggs',
  '*.egg-info',
  '.gradle',
  '.idea',
  '.vscode',
  'bin',
  'obj',
  '.dart_tool',
  '.flutter-plugins',
  '.flutter-plugins-dependencies',
  'Pods',
  'DerivedData',
  'cmake-build-*',
  '.ccache',
  '.terraform',
  '.serverless',
  '.aws-sam',
  'vendor',
  '.bundle',
  '.cargo',
]

export function defaultDisplayIgnore(): string[] {
  const style = inferShortcutStyle()
  if (style === 'mac') return [...MAC, ...WINDOWS, ...LINUX]
  if (style === 'win') return [...WINDOWS, ...MAC, ...LINUX]
  return [...LINUX, ...MAC, ...WINDOWS]
}

export function defaultOperationIgnore(): string[] {
  return [...defaultDisplayIgnore(), ...BUILD]
}
