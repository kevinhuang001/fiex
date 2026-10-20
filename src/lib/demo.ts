// Isolated UI fixture, loaded only by the explicit ?demo route; never used in the desktop app.
import type { Entry, Listing } from './types'
import { basename, join, parent } from './files'
const root = '/Users/kevin'
const now = Date.now() / 1000
function item(path: string, isDir = false, mime = 'application/octet-stream', bytes = 0): Entry {
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
  }
}
let entries = [
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
]
export async function mock(command: string, a: Record<string, unknown>): Promise<unknown> {
  switch (command) {
    case 'platform':
      return 'macos'
    case 'locations':
      return [
        { name: '个人文件夹', path: root, kind: 'favorite' },
        ...['Desktop', 'Downloads', 'Documents', 'Pictures', 'Music', 'Movies'].map((name, i) => ({
          name: ['桌面', '下载', '文稿', '图片', '音乐', '视频'][i],
          path: root + '/' + name,
          kind: 'favorite',
        })),
        { name: 'Macintosh HD', path: '/', kind: 'disk', available: 240e9, total: 500e9 },
      ]
    case 'list_directory':
      if (!entries.some((e) => e.path === a.path && e.isDir) && a.path !== root && a.path !== '/')
        throw new Error('文件夹不存在')
      return {
        path: a.path as string,
        entries: entries.filter((e) => parent(e.path) === a.path),
        warnings: [],
      } satisfies Listing
    case 'file_info':
      return entries.find((e) => e.path === a.path)
    case 'read_preview':
      return '# Fiex\n\n留一点空间，给重要的文件。\n\n支持四种视图、快速查找与空格预览。'
    case 'detect_terminals':
      return [
        { id: 'default', name: '系统默认终端' },
        { id: 'Terminal', name: 'Terminal' },
        { id: 'iTerm', name: 'iTerm2' },
      ]
    case 'native_icon':
      throw new Error('UI fixture')
    case 'search_files':
      return {
        entries: entries.filter(
          (e) =>
            e.path.startsWith(a.root as string) &&
            e.name.toLowerCase().includes((a.query as string).toLowerCase()),
        ),
        done: true,
        elapsedMs: 12,
        truncated: false,
        skipped: 0,
      }
    case 'create_entry': {
      const path = join(a.path as string, a.name as string)
      if (entries.some((e) => e.path === path)) throw new Error('同名项目已存在')
      entries.push(item(path, a.folder as boolean))
      return path
    }
    case 'rename_entry': {
      const path = join(parent(a.path as string), a.name as string)
      entries = entries.map((e) => (e.path === a.path ? { ...e, path, name: a.name as string } : e))
      return path
    }
    case 'recycle':
      entries = entries.filter((e) => !(a.paths as string[]).includes(e.path))
      return { paths: a.paths, errors: [] }
    case 'transfer':
      return { paths: [], errors: [] }
    default:
      return null
  }
}
