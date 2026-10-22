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
    linkTarget: null,
    tags: [],
    sync: null,
  }
}
function ignored(name: string, patterns: unknown): boolean {
  if (!Array.isArray(patterns)) return false
  return patterns.some((pattern) => {
    if (typeof pattern !== 'string') return false
    if (pattern.endsWith('*')) return name.startsWith(pattern.slice(0, -1))
    if (pattern.startsWith('*.')) return name.endsWith(pattern.slice(1))
    return name === pattern
  })
}

const pluginSettings = new Map<string, unknown>()
const remoteConfigs = new Map<string, { name: string; url: string }[]>()
const demoBranch = 'main'
const firstCommit = 'a'.repeat(40),
  secondCommit = 'b'.repeat(40)
const demoHistory = [
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
const demoRoots = new Set([root + '/Documents'])
const demoChanges: Record<string, string> = { [root + '/Documents/Notes.md']: ' M' }
const repoFor = (path: string) =>
  [...demoRoots]
    .sort((a, b) => b.length - a.length)
    .find((r) => path === r || path.startsWith(r + '/'))

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
entries = entries.map((entry) =>
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
  entries.push(
    item(
      root + '/Documents/zz-file-' + String(i).padStart(5, '0') + '.txt',
      false,
      'text/plain',
      120,
    ),
  )
const snapshotEntries = structuredClone(entries)
export async function mock(command: string, a: Record<string, unknown>): Promise<unknown> {
  switch (command) {
    case 'plugins_list':
      return [
        {
          id: 'org.fiex.webdav',
          name: 'WebDAV',
          version: '0.1.0',
          enabled: true,
          backendAvailable: true,
          capabilities: ['filesystem', 'connectionFactory'],
          contributes: {
            connectionFactories: [
              {
                id: 'webdav',
                title: 'WebDAV',
                fields: [
                  { id: 'url', label: '地址', type: 'url', required: true },
                  { id: 'username', label: '用户名', type: 'text' },
                  { id: 'password', label: '密码', type: 'secret' },
                  { id: 'allowInvalidCerts', label: '允许自签名证书', type: 'checkbox' },
                ],
              },
            ],
            settings: [
              { id: 'timeout', label: '连接超时（秒）', type: 'number', defaultValue: '90' },
            ],
          },
        },
        {
          id: 'org.fiex.smb',
          name: 'SMB / NAS',
          version: '0.1.0',
          enabled: true,
          backendAvailable: true,
          capabilities: ['filesystem', 'connectionFactory'],
          contributes: {
            connectionFactories: [
              {
                id: 'smb',
                title: 'SMB 网络共享',
                fields: [
                  { id: 'host', label: '主机', type: 'text', required: true },
                  { id: 'port', label: '端口', type: 'number', defaultValue: '445' },
                  { id: 'share', label: '共享名', type: 'text', required: true },
                  { id: 'base', label: '子路径', type: 'text' },
                  { id: 'username', label: '用户名', type: 'text' },
                  { id: 'password', label: '密码', type: 'secret' },
                ],
              },
            ],
          },
        },
        {
          id: 'org.fiex.git',
          name: 'Git',
          version: '0.1.0',
          enabled: true,
          backendAvailable: true,
          capabilities: ['toolbar', 'filesystem', 'commands'],
          contributes: {
            toolbar: [{ id: 'git.main', title: '源代码管理', command: 'git.status' }],
            settings: [
              { id: 'historyLimit', label: '历史提交显示条数', type: 'number', defaultValue: '50' },
            ],
          },
        },
      ]
    case 'plugin_storage_get':
      return (
        pluginSettings.get(String(a.pluginId) + ':' + String(a.scope) + ':' + String(a.key)) ?? null
      )
    case 'plugin_storage_set':
      pluginSettings.set(String(a.pluginId) + ':' + String(a.scope) + ':' + String(a.key), a.value)
      return null
    case 'plugin_storage_delete':
      pluginSettings.delete(String(a.pluginId) + ':' + String(a.scope) + ':' + String(a.key))
      return null
    case 'plugin_secret_set':
      return null
    case 'plugin_secret_available':
      return true
    case 'platform': {
      // `?platform=windows|linux` lets the browser fixture preview the
      // frameless window chrome without rebuilding the desktop app.
      const requested = new URLSearchParams(location.search).get('platform')
      return requested === 'windows' || requested === 'linux' ? requested : 'macos'
    }
    case 'locations':
      return [
        {
          name: '个人文件夹',
          path: root,
          kind: 'favorite',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        },
        ...['Desktop', 'Downloads', 'Documents', 'Pictures', 'Music', 'Movies'].map((name, i) => ({
          name: ['桌面', '下载', '文稿', '图片', '音乐', '视频'][i],
          path: root + '/' + name,
          kind: 'favorite',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        })),
        {
          name: '应用程序',
          path: root + '/Applications',
          kind: 'applications',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        },
        {
          name: 'Macintosh HD',
          path: '/',
          kind: 'disk',
          available: 240e9,
          total: 500e9,
          removable: false,
          filesystem: 'APFS',
          device: '',
          mounted: true,
          usbfs: false,
        },
      ]
    case 'list_directory': {
      if (String(a.path).startsWith('history://')) {
        const base = String(a.path).match(/^history:\/\/[^/]+\/[^/]+/)![0]
        const original = atob(base.split('/')[2]) + String(a.path).slice(base.length)
        return {
          path: a.path,
          entries: snapshotEntries
            .filter(
              (e) =>
                parent(e.path) === original &&
                (a.hidden || !e.name.startsWith('.')) &&
                !ignored(e.name, a.ignores),
            )
            .map((e) => ({
              ...e,
              path: base + e.path.slice(atob(base.split('/')[2]).length),
              readonly: true,
            })),
          warnings: [],
        }
      }

      const remoteRoot = root + '/.fiex/drives/demo-webdav'
      if ((a.path as string).startsWith(remoteRoot)) {
        return {
          path: a.path,
          entries: [
            {
              ...item(remoteRoot + '/产品截图.png', false, 'image/png', 2_400_000),
              sync: 'synced',
            },
            {
              ...item(remoteRoot + '/会议记录.md', false, 'text/markdown', 18_000),
              sync: 'synced',
            },
            {
              ...item(remoteRoot + '/素材.zip', false, 'application/zip', 45_000_000),
              sync: 'cloud',
            },
            { ...item(remoteRoot + '/2026 规划', true), sync: 'cloud' },
          ],
          warnings: [],
        }
      }

      if (!entries.some((e) => e.path === a.path && e.isDir) && a.path !== root && a.path !== '/')
        throw new Error('文件夹不存在')
      return {
        path: a.path as string,
        entries: entries.filter(
          (e) => parent(e.path) === a.path && (Boolean(a.hidden) || !e.name.startsWith('.')),
        ),
        warnings: [],
      } satisfies Listing
    }
    case 'file_info':
      return entries.find((e) => e.path === a.path)
    case 'read_preview':
      if (String(a.path).startsWith('history://'))
        return '# Fiex · 历史版本\n\n这是提交时保存的内容，当前工作区没有改变。'
      return '# Fiex\n\n留一点空间，给重要的文件。\n\n支持四种视图、快速查找与空格预览。'
    case 'detect_terminals':
      return [
        { id: 'default', name: '系统默认终端' },
        { id: 'Terminal', name: 'Terminal' },
        { id: 'iTerm', name: 'iTerm2' },
        { id: 'Tabby', name: 'Tabby' },
      ]
    case 'native_icon':
      throw new Error('UI fixture')
    case 'trash_path':
      return root + '/.Trash'
    case 'open_applications':
      return null
    case 'open_in_vscode':
      return null
    case 'detect_ides':
      return [
        { id: 'vscode', name: 'VS Code' },
        { id: 'cursor', name: 'Cursor' },
      ]
    case 'remote_connections':
      return [
        {
          id: 'demo-webdav',
          name: '团队 WebDAV',
          kind: 'webdav',
          mountPath: root + '/.fiex/drives/demo-webdav',
          host: '',
          url: 'https://dav.example.com',
          share: '',
          base: '',
        },
      ]
    case 'git_refs':
      return { head: 'aaaaaaaa', current: demoBranch, branches: ['main', 'dev'], tags: ['v0.2.4'] }
    case 'git_remote_status':
      return {
        remotes: remoteConfigs.get(String(a.root)) || [],
        branch: demoBranch,
        upstream: '',
        ahead: 0,
        behind: 0,
      }
    case 'git_remote_save': {
      const previous = remoteConfigs.get(String(a.root)) || []
      remoteConfigs.set(String(a.root), [
        ...previous.filter((r) => r.name !== a.name),
        { name: String(a.name), url: String(a.url) },
      ])
      return null
    }
    case 'git_sync':
      return a.push ? 'Pushed' : 'Already up to date'
    case 'git_identity':
      return { name: 'Kevin Huang', email: 'kevin@example.com' }
    case 'git_init': {
      const path = String(a.path)
      demoRoots.add(path)
      for (const entry of entries)
        if (!entry.isDir && entry.path.startsWith(path + '/')) demoChanges[entry.path] = '??'
      return {
        root: path,
        branch: 'main',
        entries: Object.fromEntries(
          Object.entries(demoChanges).filter(([p]) => p.startsWith(path + '/')),
        ),
      }
    }
    case 'git_stage':
    case 'git_unstage': {
      for (const path of Object.keys(demoChanges)) {
        if (
          !path.startsWith(String(a.root) + '/') ||
          (!a.all && !(a.paths as string[]).includes(path))
        )
          continue
        const code = demoChanges[path]
        demoChanges[path] =
          command === 'git_stage'
            ? code === '??'
              ? 'A '
              : `${code[1] === ' ' ? code[0] : code[1]} `
            : code[0] === 'A'
              ? '??'
              : ` ${code[0]}`
      }
      return null
    }
    case 'git_commit': {
      if (!String(a.message).trim()) throw new Error('请填写提交说明')
      const staged = Object.keys(demoChanges).filter(
        (path) =>
          path.startsWith(String(a.root) + '/') && ![' ', '?'].includes(demoChanges[path][0]),
      )
      if (!staged.length) throw new Error('没有暂存的更改')
      for (const path of staged) delete demoChanges[path]
      return '8e4a9c2'
    }
    case 'git_history':
      return demoHistory.slice(Number(a.skip) || 0, (Number(a.skip) || 0) + 50)
    case 'git_browse':
      return (
        'history://' +
        btoa(String(a.root)) +
        '/' +
        (String(a.reference) === 'dev'
          ? secondCommit
          : String(a.reference).length === 40
            ? String(a.reference)
            : firstCommit)
      )
    case 'git_status': {
      if (String(a.path).startsWith('history://'))
        return {
          root: atob(String(a.path).split('/')[2]),
          branch: String(a.path).split('/')[3].slice(0, 8),
          entries: {},
        }
      const repo = repoFor(String(a.path))
      return repo
        ? {
            root: repo,
            branch: demoBranch,
            entries: Object.fromEntries(
              Object.entries(demoChanges).filter(([p]) => p.startsWith(repo + '/')),
            ),
          }
        : null
    }
    case 'set_tags': {
      const tags = a.tags as { name: string; color: number }[]
      entries = entries.map((entry) =>
        (a.paths as string[]).includes(entry.path) ? { ...entry, tags } : entry,
      )
      return null
    }
    case 'search_files':
      return {
        entries: entries.filter(
          (e) =>
            e.path.startsWith(a.root as string) &&
            e.name.toLowerCase().includes((a.query as string).toLowerCase()) &&
            !ignored(e.name, a.ignores),
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
      if (!a.folder && repoFor(path)) demoChanges[path] = '??'
      return path
    }
    case 'rename_entry': {
      const path = join(parent(a.path as string), a.name as string)
      entries = entries.map((e) => (e.path === a.path ? { ...e, path, name: a.name as string } : e))
      const previous = String(a.path)
      if (demoChanges[previous]) {
        demoChanges[path] = demoChanges[previous]
        delete demoChanges[previous]
      }
      return path
    }
    case 'recycle':
      entries = entries.filter((e) => !(a.paths as string[]).includes(e.path))
      return { paths: a.paths, errors: [], skipped: 0 }
    case 'transfer': {
      const planned: { source: string; destination: string; action: string }[] = []
      const conflicts = []
      for (const source of a.paths as string[]) {
        let destination = join(String(a.destination), basename(source))
        if (destination.startsWith(source + '/') || (a.moving && destination === source)) continue
        const incoming = entries.find((e) => e.path === source)
        const existing = entries.find((e) => e.path === destination)
        const key = source + '\0' + destination
        let action =
          (a.resolutions as { key: string; action: string }[] | undefined)?.find(
            (r) => r.key === key,
          )?.action || ''
        if (a.keepBoth) action = 'keepBoth'
        if (existing && incoming && !action) {
          conflicts.push({ key, source: incoming, destination: existing })
          continue
        }
        if (action === 'skip') continue
        if (existing && action === 'keepBoth') {
          const name = basename(source),
            dot = name.lastIndexOf('.')
          const stem = dot > 0 && !incoming?.isDir ? name.slice(0, dot) : name
          const ext = dot > 0 && !incoming?.isDir ? name.slice(dot) : ''
          for (let index = 2; ; index++) {
            const candidate = join(String(a.destination), `${stem} (${index})${ext}`)
            if (!entries.some((e) => e.path === candidate)) {
              destination = candidate
              break
            }
          }
        }
        planned.push({ source, destination, action })
      }
      if (conflicts.length) return { paths: [], errors: [], skipped: 0, conflicts }
      const paths: string[] = []
      for (const { source, destination, action } of planned) {
        const selected = entries.filter((e) => e.path === source || e.path.startsWith(source + '/'))
        const transferred = selected.map((e) => ({
          ...e,
          path: destination + e.path.slice(source.length),
          name: e.path === source ? basename(destination) : e.name,
        }))
        if (action === 'replace')
          entries = entries.filter(
            (e) => e.path !== destination && !e.path.startsWith(destination + '/'),
          )
        if (a.moving) entries = entries.filter((e) => !selected.includes(e))
        entries.push(...transferred)
        paths.push(destination)
      }
      return { paths, errors: [], skipped: 0, conflicts: [] }
    }

    case 'file_checksums':
      // Known abc vectors for the isolated UI fixture.
      return [
        {
          algorithm: 'SHA-256',
          value: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
        },
        {
          algorithm: 'SHA-512',
          value:
            'ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f',
        },
        { algorithm: 'SHA-1', value: 'a9993e364706816aba3e25717850c26c9cd0d89d' },
        { algorithm: 'MD5', value: '900150983cd24fb0d6963f7d28e17f72' },
        { algorithm: 'CRC32', value: '352441c2' },
      ]
    default:
      return null
  }
}
