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
interface DemoPlugin {
  id: string
  name: string
  version: string
  description?: string
  enabled: boolean
  approved: boolean
  backendAvailable: boolean
  capabilities: string[]
  declaredPermissions: string[]
  grantedPermissions: string[]
  requestedNetworkDomains: string[]
  allowedNetworkDomains: string[]
  requestedCommands?: string[]
  allowedCommands?: string[]
  author?: { name: string; url?: string }
  contributions?: Record<string, unknown>
}
const fixturePlugin: DemoPlugin = {
  id: 'org.test.fixture',
  name: '测试插件',
  version: '1.0.0',
  description: '用于验证右键子菜单、隔离 iframe 与上下文桥。',
  enabled: true,
  approved: true,
  backendAvailable: true,
  capabilities: ['commands', 'contextMenu', 'panel', 'search'],
  declaredPermissions: [],
  grantedPermissions: [],
  requestedNetworkDomains: [],
  allowedNetworkDomains: [],
  author: { name: 'Fiex Fixtures' },
  contributions: {
    toolbar: [
      {
        id: 'fixture.tool',
        title: '测试工具',
        icon: 'puzzle-piece',
        panel: { entry: 'ui/index.html', title: '测试工具' },
      },
    ],
    search: [
      {
        id: 'fixture.search',
        title: 'Fixture Index',
        icon: 'magnifying-glass',
        scope: 'global',
        command: 'search.query',
      },
    ],
    contextMenus: [
      {
        id: 'fixture.root',
        title: '测试插件',
        when: 'context.selection.count > 0',
        children: [
          {
            id: 'fixture.panel',
            title: '信息面板',
            panel: {
              entry: 'ui/index.html',
              title: '信息面板',
              defaultSize: { width: 620, height: 560 },
            },
          },
          { id: 'fixture.action', title: '检查文件', command: 'fixture.inspect' },
        ],
      },
    ],
  },
}
const fixturePlugins: DemoPlugin[] = [
  fixturePlugin,
  {
    id: 'org.example.git',
    name: 'Git',
    version: '0.1.0',
    description: '源代码管理、提交历史、远程仓库与只读历史浏览。',
    enabled: true,
    approved: true,
    backendAvailable: true,
    capabilities: ['filesystem', 'toolbar', 'commands'],
    declaredPermissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
      'network',
      'commands',
    ],
    grantedPermissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
      'network',
      'commands',
    ],
    requestedNetworkDomains: ['github.com'],
    allowedNetworkDomains: ['github.com'],
    requestedCommands: ['git'],
    allowedCommands: ['git'],
    author: { name: 'Fiex Contributors' },
    contributions: {
      toolbar: [
        {
          id: 'git.main',
          title: '源代码管理',
          icon: 'git-branch',
          command: 'git.status',
          schemes: ['history'],
        },
      ],
      settings: [
        { id: 'historyLimit', label: '历史提交显示条数', type: 'number', defaultValue: '50' },
      ],
    },
  },
  {
    id: 'org.example.checksum',
    name: '校验和',
    version: '0.1.0',
    description: '计算 SHA-256、SHA-512、SHA-1、MD5 与 CRC32。',
    enabled: true,
    approved: true,
    backendAvailable: true,
    capabilities: ['commands', 'contextMenu'],
    declaredPermissions: ['folders.read', 'files.read'],
    grantedPermissions: ['folders.read', 'files.read'],
    requestedNetworkDomains: [],
    allowedNetworkDomains: [],
    author: { name: 'Fiex Contributors' },
    contributions: {
      contextMenus: [
        {
          id: 'checksum.calculate',
          title: '查看校验和',
          when: 'context.selection.isFile',
          command: 'checksum.calculate',
          resultView: 'checksum',
        },
      ],
    },
  },
]
const query = new URLSearchParams(location.search)
let demoGitEntries: Record<string, string> = {
  '/Users/kevin/Documents/Notes.md': ' M',
}
const officialMode =
  query.has('officialPlugins') ||
  (import.meta.env.DEV && !query.has('demo') && !query.has('pluginFixture'))
const archiveFormats = [
  ['zip', 'ZIP'],
  ['rar', 'RAR'],
  ['7z', '7z'],
  ['tar', 'TAR'],
  ['tar.gz', 'TAR.GZ'],
  ['tar.bz2', 'TAR.BZ2'],
  ['tar.xz', 'TAR.XZ'],
] as const
const archiveItems = (picker: boolean) =>
  archiveFormats.map(([format, title]) => ({
    id: `archive.${picker ? 'to.' : ''}${format}`,
    title,
    command: 'archive.compress',
    args: { format },
    ...(picker
      ? {
          picker: {
            mode: 'saveFile',
            suggestedName: `Archive.${format}`,
            extensions: [format.split('.').pop()],
            param: 'destination',
          },
        }
      : {}),
  }))
const officialPlugins: DemoPlugin[] = officialMode
  ? [
      {
        id: 'org.fiex.git',
        name: 'Git',
        version: '0.2.7',
        description: '源代码管理、提交历史、远程仓库与只读历史浏览。',
        enabled: true,
        approved: true,
        backendAvailable: true,
        capabilities: ['filesystem', 'toolbar', 'panel'],
        declaredPermissions: [
          'folders.read',
          'folders.create',
          'folders.delete',
          'folders.rename',
          'files.read',
          'files.write',
          'network',
          'commands',
        ],
        grantedPermissions: [
          'folders.read',
          'folders.create',
          'folders.delete',
          'folders.rename',
          'files.read',
          'files.write',
          'network',
          'commands',
        ],
        requestedCommands: ['git'],
        allowedCommands: ['git'],
        requestedNetworkDomains: ['github.com'],
        allowedNetworkDomains: ['github.com'],
        author: { name: 'Fiex Contributors' },
        contributions: {
          toolbar: [
            {
              id: 'git.main',
              title: '源代码管理',
              icon: 'git-branch',
              schemes: ['history'],
              panel: {
                entry: 'ui/dist/index.html',
                title: '源代码管理',
                defaultSize: { width: 480, height: 300 },
              },
            },
          ],
          settings: [
            { id: 'historyLimit', label: '历史提交显示条数', type: 'number', defaultValue: '50' },
          ],
        },
      },
      {
        id: 'org.fiex.archive',
        name: '压缩与解压',
        version: '0.2.7',
        description: '压缩与解压 ZIP、RAR、7z、TAR 等格式。',
        enabled: true,
        approved: true,
        backendAvailable: true,
        capabilities: ['contextMenu', 'panel'],
        declaredPermissions: [
          'folders.read',
          'folders.create',
          'folders.delete',
          'folders.rename',
          'files.read',
          'files.write',
        ],
        grantedPermissions: [
          'folders.read',
          'folders.create',
          'folders.delete',
          'folders.rename',
          'files.read',
          'files.write',
        ],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        contributions: {
          contextMenus: [
            {
              id: 'archive.compress',
              title: '压缩',
              when: 'context.selection.isNotArchive',
              children: [
                ...archiveItems(false),
                {
                  id: 'archive.advanced',
                  title: '高级压缩…',
                  panel: {
                    entry: 'ui/dist/index.html',
                    title: '高级压缩',
                    defaultSize: { width: 400, height: 420 },
                  },
                },
              ],
            },
            {
              id: 'archive.extract',
              title: '解压缩',
              when: 'context.selection.isArchive',
              command: 'archive.extract',
            },
          ],
          settings: [
            {
              id: 'compressIgnores',
              label: '压缩时忽略的文件或目录',
              type: 'list',
              defaultValue: ['.git', 'node_modules'],
            },
            {
              id: 'extractIgnores',
              label: '解压时忽略的文件或目录',
              type: 'list',
              defaultValue: ['__MACOSX'],
            },
          ],
        },
      },
      {
        id: 'org.fiex.gpg',
        name: 'GPG',
        version: '0.1.0',
        description: '使用本机 GnuPG 加密、解密、签名、验签及管理公钥。',
        enabled: true,
        approved: true,
        backendAvailable: true,
        capabilities: ['contextMenu', 'panel'],
        declaredPermissions: ['commands'],
        grantedPermissions: ['commands'],
        requestedCommands: ['gpg'],
        allowedCommands: ['gpg'],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        contributions: {
          toolbar: [
            {
              id: 'gpg.keys',
              title: '公钥管理',
              icon: 'key-round',
              panel: {
                entry: 'ui/dist/index.html',
                title: 'GPG 公钥管理',
                defaultSize: { width: 420, height: 460 },
              },
            },
          ],
          contextMenus: [
            {
              id: 'gpg.menu',
              title: 'GPG',
              when: 'context.selection.count > 0',
              children: [
                {
                  id: 'gpg.encrypt',
                  title: '加密…',
                  panel: {
                    entry: 'ui/dist/index.html',
                    title: 'GPG 加密',
                    defaultSize: { width: 560, height: 520 },
                  },
                },
                {
                  id: 'gpg.decrypt',
                  title: '解密…',
                  panel: {
                    entry: 'ui/dist/index.html',
                    title: 'GPG 解密',
                    defaultSize: { width: 520, height: 400 },
                  },
                },
                {
                  id: 'gpg.sign',
                  title: '签名…',
                  panel: {
                    entry: 'ui/dist/index.html',
                    title: 'GPG 签名',
                    defaultSize: { width: 540, height: 470 },
                  },
                },
                {
                  id: 'gpg.verify',
                  title: '验证签名…',
                  when: 'context.selection.count == 1',
                  panel: {
                    entry: 'ui/dist/index.html',
                    title: '验证签名',
                    defaultSize: { width: 520, height: 420 },
                  },
                },
              ],
            },
          ],
        },
      },
      {
        id: 'org.fiex.checksum',
        name: '校验和',
        version: '0.2.7',
        description: '计算 SHA-256、SHA-512、SHA-1、MD5 与 CRC32。',
        enabled: true,
        approved: true,
        backendAvailable: true,
        capabilities: ['contextMenu', 'panel'],
        declaredPermissions: ['folders.read', 'files.read'],
        grantedPermissions: ['folders.read', 'files.read'],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        contributions: {
          contextMenus: [
            {
              id: 'checksum.calculate',
              title: '查看校验和',
              when: 'context.selection.isFile',
              panel: {
                entry: 'ui/dist/index.html',
                title: '校验和',
                defaultSize: { width: 540, height: 300 },
              },
            },
          ],
        },
      },
    ]
  : []
const demoGpgKeys = [
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
const demoPlugins: DemoPlugin[] = officialMode
  ? structuredClone(officialPlugins)
  : query.has('pluginFixture')
    ? structuredClone(fixturePlugins)
    : []
const marketplace = [
  {
    id: 'org.example.git',
    name: 'Git',
    version: '0.1.0',
    description: '源代码管理、提交历史、远程仓库与只读历史浏览。',
    author: { name: 'Fiex Contributors' },
    permissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
      'network',
    ],
    requestedNetworkDomains: ['github.com'],
    asset: 'git-linux-x86_64.fiexplugin',
    repository: 'kevinhuang001/fiex-official-plugins',
    contributions: {
      toolbar: [
        {
          id: 'git.main',
          title: '源代码管理',
          icon: 'git-branch',
          command: 'git.status',
          schemes: ['history'],
        },
      ],
      settings: [
        { id: 'historyLimit', label: '历史提交显示条数', type: 'number', defaultValue: '50' },
      ],
    },
    capabilities: ['filesystem', 'toolbar', 'commands'],
  },
  {
    id: 'org.example.webdav',
    name: 'WebDAV',
    version: '0.1.0',
    description: '连接 WebDAV 服务器，目录按需读取，不落地零字节占位文件。',
    author: { name: 'Fiex Contributors' },
    permissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
      'network',
    ],
    requestedNetworkDomains: [],
    commands: [],
    asset: 'webdav-linux-x86_64.fiexplugin',
    repository: 'kevinhuang001/fiex-official-plugins',
    capabilities: ['filesystem', 'connection'],
    contributions: {
      connections: [
        {
          id: 'webdav',
          title: 'WebDAV',
          fields: [
            { id: 'url', label: '服务器地址', type: 'url', required: true },
            { id: 'username', label: '用户名', type: 'text' },
            { id: 'password', label: '密码', type: 'secret' },
          ],
        },
      ],
    },
  },
  {
    id: 'org.example.smb',
    name: 'SMB / NAS',
    version: '0.1.0',
    description: '通过独立进程连接 SMB / NAS 共享。',
    author: { name: 'Fiex Contributors' },
    permissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
      'network',
    ],
    requestedNetworkDomains: [],
    commands: [],
    asset: 'smb-linux-x86_64.fiexplugin',
    repository: 'kevinhuang001/fiex-official-plugins',
    capabilities: ['filesystem', 'connection'],
    contributions: {
      connections: [
        {
          id: 'smb',
          title: 'SMB 网络共享',
          fields: [
            { id: 'host', label: '主机', type: 'text', required: true },
            { id: 'share', label: '共享名', type: 'text', required: true },
            { id: 'username', label: '用户名', type: 'text' },
            { id: 'password', label: '密码', type: 'secret' },
          ],
        },
      ],
    },
  },
  {
    id: 'org.example.archive',
    name: '压缩与解压',
    version: '0.1.0',
    description: '压缩与解压 ZIP、7z、TAR 等格式。',
    author: { name: 'Fiex Contributors' },
    permissions: [
      'folders.read',
      'folders.create',
      'folders.delete',
      'folders.rename',
      'files.read',
      'files.write',
    ],
    requestedNetworkDomains: [],
    asset: 'archive-linux-x86_64.fiexplugin',
    repository: 'kevinhuang001/fiex-official-plugins',
    capabilities: ['commands', 'toolbar', 'contextMenu'],
    contributions: {
      contextMenus: [
        {
          id: 'archive.compress',
          title: '压缩为…',
          when: 'context.selection.count > 0',
          children: [
            {
              id: 'archive.zip',
              title: 'ZIP',
              command: 'archive.compress',
              args: { format: 'zip' },
            },
          ],
        },
      ],
    },
  },
  {
    id: 'org.example.checksum',
    name: '校验和',
    version: '0.1.0',
    description: '计算 SHA-256、SHA-512、SHA-1、MD5 与 CRC32。',
    author: { name: 'Fiex Contributors' },
    permissions: ['folders.read', 'files.read'],
    requestedNetworkDomains: [],
    asset: 'checksum-linux-x86_64.fiexplugin',
    repository: 'kevinhuang001/fiex-official-plugins',
    capabilities: ['commands', 'contextMenu'],
    contributions: {
      contextMenus: [
        {
          id: 'checksum.calculate',
          title: '查看校验和',
          when: 'context.selection.isFile',
          command: 'checksum.calculate',
          resultView: 'checksum',
        },
      ],
    },
  },
]
const preparedInstalls = new Map<string, (typeof marketplace)[number]>()

export async function mock(command: string, a: Record<string, unknown>): Promise<unknown> {
  switch (command) {
    case 'plugins_list':
      return structuredClone(demoPlugins)
    case 'plugin_errors':
      return []
    case 'plugin_error_report':
      return null
    case 'plugin_error_clear':
      return null
    case 'plugin_panel_html': {
      if (officialMode) {
        const slug =
          String(a.pluginId || '')
            .split('.')
            .pop() || ''
        const response = await fetch(`/__fiex_official_plugin/${slug}`)
        if (!response.ok) throw new Error(await response.text())
        return await response.text()
      }
      if (query.has('pluginFixture'))
        return "<html><body><h2>示例插件面板</h2><div id=\"selected\">连接中</div>\n<script>\nwindow.addEventListener('message', (event) => {\n if(event.data?.type==='fiex.bridge.probe'){window.parent.postMessage({type:'fiex.bridge.listening'},'*');return;}\n if(event.data?.type!=='fiex.bridge.init'||!event.ports?.[0])return;\n const port=event.ports[0];\n port.onmessage=(message)=>{\n   if(message.data?.type==='response') {\n     const p=message.data?.result?.selectionPaths||[];\n     document.getElementById('selected').textContent=p.join(',');\n   }\n };\n port.start();\n port.postMessage({type:'fiex.bridge.ready',bridgeVersion:'1.0',sessionId:event.data.sessionId,nonce:event.data.nonce});\n port.postMessage({type:'request',bridgeVersion:'1.0',sessionId:event.data.sessionId,requestId:'fixture-1',method:'context.get',params:{}});\n});\n</script></body></html>"
      throw new Error('No installed fixture plugin')
    }
    case 'plugin_invoke': {
      const method = String(a.method || '')
      const params = (a.params || {}) as Record<string, unknown>
      if (officialMode) {
        if (method === 'git.status')
          return {
            root: '/Users/kevin/Documents',
            branch: 'main',
            entries: { ...demoGitEntries },
          }
        if (method === 'git.refs')
          return { head: 'bbbbbbbb', current: 'main', branches: ['main'], tags: [] }
        if (method === 'git.history') return demoHistory
        if (method === 'gpg.info') return { version: 'gpg (GnuPG) 2.4.5' }
        if (method === 'gpg.keys') return demoGpgKeys
        if (method === 'gpg.verify')
          return {
            valid: true,
            signer: demoGpgKeys[0].name,
            fingerprint: demoGpgKeys[0].fingerprint,
          }
        if (method === 'gpg.import') return { imported: 1, status: 'demo' }
        if (method.startsWith('gpg.')) return { ok: true, status: 'demo' }
        if (method === 'git.identity') return { name: 'Kevin Huang', email: 'kevin@example.com' }
        if (method === 'git.remoteStatus')
          return {
            remotes: [{ name: 'origin', url: 'https://github.com/kevinhuang001/fiex.git' }],
            branch: 'main',
            upstream: 'origin/main',
            ahead: 0,
            behind: 0,
          }
        if (method === 'git.commit') return 'ccccccc'
        if (method === 'git.init' || method === 'git.stage' || method === 'git.unstage') return null
        if (method === 'git.remoteSave' || method === 'git.remoteSync') return null
        if (method === 'archive.compress') return '/Users/kevin/Documents/Archive.zip'
        if (method === 'checksum.calculate')
          return {
            path: params.path,
            size: 2451,
            sha256: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
            sha512:
              'ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f',
            sha1: 'a9993e364706816aba3e25717850c26c9cd0d89d',
            md5: '900150983cd24fb0d6963f7d28e17f72',
            crc32: '352441c2',
          }
      }
      if (method === 'checksum.calculate')
        return [
          {
            algorithm: 'SHA-256',
            value: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
          },
        ]
      return { ok: true, method, params }
    }
    case 'plugin_decorations': {
      if (!officialMode) return []
      const entries = (a.entries || []) as Array<{ path: string; extension?: string }>
      return entries.flatMap((entry) => {
        const extension = (entry.extension || '').toLowerCase()
        if (entry.path.endsWith('/Notes.md'))
          return [
            {
              path: entry.path,
              badges: [],
              fields: {
                name: {
                  badges: [{ text: 'M', color: 'orange', tooltip: 'Git：已修改' }],
                  style: { color: '#c2710c', fontWeight: '600' },
                },
                modified: { style: { color: '#c2710c' } },
                size: { badges: [{ text: '新', color: 'green' }] },
                kind: { style: { color: '#2f6f9f' } },
                row: { style: { background: '#fff9f0' } },
              },
              icon: 'fiex-resource://org.example.icon-theme/markdown',
              iconPath: '/plugin-icons/plugin-markdown.svg',
              providers: ['Git'],
            },
          ]
        if (['zip', 'rar', '7z', 'gz', 'bz2', 'xz', 'tar'].includes(extension))
          return [
            {
              path: entry.path,
              badges: [{ text: extension.toUpperCase().slice(0, 3), color: 'neutral' }],
              providers: ['Archive'],
            },
          ]
        return []
      })
    }
    case 'plugin_file_dialog':
      if (a.mode === 'saveFile') return '/Users/kevin/Documents/Archive.zip'
      if (a.mode === 'openFile') return '/Users/kevin/Documents/Archive.zip'
      return '/Users/kevin/Documents'
    case 'plugin_prepare_local_install':
      return {
        ticket: 'demo-local-install',
        id: 'org.example.local',
        name: '本地示例插件',
        version: '1.2.3',
        author: { name: 'Example Author' },
        permissions: ['folders.read', 'files.read'],
        requestedNetworkDomains: [],
        commands: [],
        signed: true,
        trusted: false,
        publisher: 'example',
        publisherName: 'Example Publisher',
        keyId: 'demo-key',
        fingerprint: 'AB12 CD34 EF56 7890',
      }
    case 'plugin_files_dialog':
      return ['/Users/kevin/Documents/Notes.md']
    case 'plugin_network_domains':
      return ['github.com']
    case 'plugin_permission_allowed':
      return new URLSearchParams(location.search).has('pluginFixture')
    case 'file_checksums':
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
    case 'plugin_search':
      if (String(a.query).toLowerCase().includes('plugin-index-hit'))
        return [
          {
            path: '/Users/kevin/Documents/Indexed result.txt',
            name: 'Indexed result.txt',
            isDir: false,
            isSymlink: false,
            size: 42,
            modified: 1770000000,
            created: 1770000000,
            extension: 'txt',
            mime: 'text/plain',
            readonly: false,
            linkTarget: null,
            tags: [],
            sync: null,
          },
        ]
      return []
    case 'plugin_marketplace':
      return marketplace
    case 'plugin_release_assets':
      return marketplace.map((entry) => entry.asset)
    case 'plugin_prepare_install': {
      const entry = marketplace.find((item) => item.asset === a.assetName)
      if (!entry) throw new Error('插件包不存在')
      const ticket = entry.id
      preparedInstalls.set(ticket, entry)
      return {
        ticket,
        id: entry.id,
        name: entry.name,
        version: entry.version,
        author: entry.author,
        permissions: entry.permissions,
        requestedNetworkDomains: entry.requestedNetworkDomains,
        commands: [],
        repository: entry.repository,
        signed: true,
        trusted: true,
        publisher: 'org.fiex',
        publisherName: 'Fiex Contributors',
        keyId: 'demo',
        fingerprint: 'DEMO OFFICIAL KEY',
      }
    }
    case 'plugin_confirm_install': {
      const entry = preparedInstalls.get(String(a.ticket))
      if (!entry) throw new Error('安装预览已过期')
      preparedInstalls.delete(String(a.ticket))
      if (!demoPlugins.some((plugin) => plugin.id === entry.id)) {
        demoPlugins.push({
          id: entry.id,
          name: entry.name,
          version: entry.version,
          description: entry.description,
          enabled: false,
          approved: true,
          backendAvailable: true,
          capabilities: entry.capabilities || [],
          declaredPermissions: entry.permissions,
          grantedPermissions: entry.permissions,
          requestedNetworkDomains: entry.requestedNetworkDomains,
          allowedNetworkDomains: entry.requestedNetworkDomains,
          author: entry.author,
          contributions: entry.contributions || {},
        })
      }
      return entry.id
    }
    case 'plugin_cancel_install':
      preparedInstalls.delete(String(a.ticket))
      return null
    case 'plugin_uninstall': {
      const index = demoPlugins.findIndex((plugin) => plugin.id === a.pluginId)
      if (index >= 0) demoPlugins.splice(index, 1)
      return null
    }
    case 'plugin_activate': {
      const plugin = demoPlugins.find((item) => item.id === a.pluginId)
      const consent = a.consent as { permissions: string[]; domains: string[]; commands: string[] }
      if (plugin) {
        if (
          !(plugin.declaredPermissions || []).every((value) => consent.permissions.includes(value))
        )
          throw new Error('请逐项同意全部权限')
        plugin.approved = true
        plugin.enabled = true
        plugin.grantedPermissions = [...consent.permissions]
        plugin.allowedNetworkDomains = [...consent.domains]
        plugin.allowedCommands = [...consent.commands]
      }
      return null
    }
    case 'plugin_set_enabled': {
      const plugin = demoPlugins.find((item) => item.id === a.pluginId)
      if (plugin) plugin.enabled = Boolean(a.enabled)
      return null
    }
    case 'plugin_set_permission': {
      const plugin = demoPlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        const permission = String(a.permission)
        plugin.grantedPermissions = a.granted
          ? [...new Set([...plugin.grantedPermissions, permission])]
          : plugin.grantedPermissions.filter((value) => value !== permission)
        if (!a.granted) plugin.enabled = false
      }
      return null
    }
    case 'plugin_set_command': {
      const plugin = demoPlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        plugin.allowedCommands = a.granted
          ? [...new Set([...(plugin.allowedCommands || []), String(a.command)])]
          : (plugin.allowedCommands || []).filter((value) => value !== a.command)
        if (!a.granted) plugin.enabled = false
      }
      return null
    }
    case 'plugin_set_network_domain': {
      const plugin = demoPlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        const domain = String(a.domain)
        plugin.allowedNetworkDomains = a.granted
          ? [...new Set([...plugin.allowedNetworkDomains, domain])]
          : plugin.allowedNetworkDomains.filter((value) => value !== domain)
      }
      return null
    }
    case 'plugin_approve_install': {
      const plugin = demoPlugins.find((item) => item.id === a.pluginId)
      if (plugin) {
        plugin.approved = true
        plugin.enabled = false
        plugin.grantedPermissions = []
      }
      return null
    }
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
      if (String(a.path).endsWith('/Palette.json'))
        return '{\n  "accent": "#387ca8",\n  "surface": "#f8fafc",\n  "radius": 12\n}'
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
          pluginId: 'org.example.webdav',
          mountPath: root + '/.fiex/drives/demo-webdav',
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
      const progress = a.onProgress as
        { onmessage?: (value: { done: number; total: number; step?: string }) => void } | undefined
      const files = a.paths as string[]
      if (progress?.onmessage && files.length) {
        const steps = 3
        for (let index = 1; index <= steps; index += 1) {
          progress.onmessage({
            done: index,
            total: steps,
            step: basename(files[Math.min(index - 1, files.length - 1)]),
          })
          await new Promise((resolve) => setTimeout(resolve, 120))
        }
      }
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
    default:
      return null
  }
}
