// Demo file tree, history and static fixtures for the isolated ?demo route.
import type { Entry } from '../types'
import { basename, join } from '../files'

export const root = '/Users/kevin'
const now = Date.now() / 1000
export function item(
  path: string,
  isDir = false,
  mime = 'application/octet-stream',
  bytes = 0,
): Entry {
  return {
    path,
    name: basename(path),
    isDir,
    isSymlink: false,
    size: bytes,
    modified: now - 3600,
    created: now - 86400,
    extension: isDir ? '' : path.split('.').pop()!,
    mime,
    readonly: false,
    linkTarget: null,
    tags: [],
    sync: null,
  }
}
export function ignored(name: string, patterns: unknown): boolean {
  if (!Array.isArray(patterns)) return false
  return patterns.some((pattern) => {
    if (typeof pattern !== 'string') return false
    if (pattern.endsWith('*')) return name.startsWith(pattern.slice(0, -1))
    if (pattern.startsWith('*.')) return name.endsWith(pattern.slice(1))
    return name === pattern
  })
}

export const demoBranch = 'main'
export const firstCommit = 'a'.repeat(40),
  secondCommit = 'b'.repeat(40)
export const demoHistory = [
  {
    id: secondCommit,
    shortId: 'bbbbbbbb',
    refs: 'dev',
    subject: '完善文件浏览与拖拽',
    author: 'Kevin Huang',
    timestamp: now - 3600,
  },
  {
    id: firstCommit,
    shortId: 'aaaaaaaa',
    refs: 'main',
    subject: '初始版本',
    author: 'Kevin Huang',
    timestamp: now - 86400,
  },
]
export const demoRoots = new Set([root + '/Documents'])
export const demoChanges: Record<string, string> = { [root + '/Documents/Notes.md']: ' M' }
export const repoFor = (path: string) =>
  [...demoRoots]
    .sort((a, b) => b.length - a.length)
    .find((r) => path === r || path.startsWith(r + '/'))

export let demoEntries = [
  ...['Desktop', 'Documents', 'Downloads', 'Pictures', 'Music', 'Movies'].map((x) =>
    item(join(root, x), true),
  ),
  item(root + '/Documents/Design', true),
  item(root + '/Documents/Projects', true),
  item(root + '/Documents/Research', true),
  item(root + '/Documents/Reading', true),
  item(root + '/Documents/Fiex — 产品设计.pdf', false, 'application/pdf', 3428192),
  item(root + '/Documents/Notes.md', false, 'text/markdown', 2451),
  item(root + '/Documents/Palette.json', false, 'application/json', 882),
  item(root + '/Documents/Archive.zip', false, 'application/zip', 1428890),
  item(root + '/Documents/Meeting.txt', false, 'text/plain', 6280),
  item(
    root + '/Documents/Budget.xlsx',
    false,
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    42771,
  ),
  item(root + '/Documents/Research/InSb.pdf', false, 'application/pdf', 2000000),
  item(
    root + '/Documents/季度报告.docx',
    false,
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    18432,
  ),
  item(
    root + '/Documents/预算表.xlsx',
    false,
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    9216,
  ),
  item(
    root + '/Documents/产品介绍.pptx',
    false,
    'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    15360,
  ),
  item(root + '/Documents/示例数据库.db', false, 'application/vnd.sqlite3', 147456),
  item(root + '/Documents/预算明细.csv', false, 'text/csv', 4096),
  item(root + '/Documents/Research/未知格式.xyz', false, 'application/octet-stream', 7340032),
  item(root + '/Documents/Projects/fiex', true),
  item(root + '/Documents/Projects/App.tsx', false, 'text/plain', 18342),
  item(root + '/Documents/Projects/main.rs', false, 'text/plain', 4210),
  item(root + '/Documents/Projects/server.py', false, 'text/plain', 9120),
  item(root + '/Documents/Projects/theme.css', false, 'text/css', 3301),
  item(root + '/Documents/Projects/docker-compose.yml', false, 'text/yaml', 782),
  item(root + '/Documents/Projects/README.md', false, 'text/markdown', 2401),
  item(root + '/Documents/Projects/logo.png', false, 'image/png', 48211),
  item(root + '/Applications', true),
  item(root + '/Applications/Fiex.app', true),
  item(root + '/.Trash', true),
  item(root + '/.Trash/Old draft.txt', false, 'text/plain', 1200),
]
demoEntries = demoEntries.map((entry) =>
  entry.name === 'Notes.md'
    ? {
        ...entry,
        tags: [
          { name: '重要', color: 6 },
          { name: '工作', color: 4 },
        ],
      }
    : entry,
)
// Large-directory fixture for UI responsiveness regression tests and recording.
const bulkCount = Math.min(
  20000,
  Math.max(0, Number(new URLSearchParams(location.search).get('count')) || 0),
)
for (let i = 0; i < bulkCount; i++)
  demoEntries.push(
    item(
      root + '/Documents/zz-file-' + String(i).padStart(5, '0') + '.txt',
      false,
      'text/plain',
      120,
    ),
  )
export const snapshotEntries = structuredClone(demoEntries)

export let demoGitEntries: Record<string, string> = {
  '/Users/kevin/Documents/Notes.md': ' M',
}

export const demoGpgKeys = [
  {
    fingerprint: '0123456789ABCDEF0123456789ABCDEF01234567',
    keyId: '01234567',
    name: 'Ada Lovelace <ada@example.org>',
    email: 'ada@example.org',
    expires: '',
  },
  {
    fingerprint: 'AAAABBBBCCCCDDDDEEEEFFFF00001111222233334444',
    keyId: '22223333',
    name: 'Fiex Release Signing <release@fiex.app>',
    email: 'release@fiex.app',
    expires: '',
  },
]
