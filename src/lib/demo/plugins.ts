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

export const fixturePlugins: DemoPlugin[] = [
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

export const query = new URLSearchParams(location.search)
export const officialMode =
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

export const officialPlugins: DemoPlugin[] = officialMode
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
        id: 'org.fiex.office-preview',
        name: 'Office 预览',
        version: '0.2.8',
        description: 'Word、Excel、PowerPoint 的只读预览（文本级）。',
        enabled: true,
        approved: true,
        backendAvailable: true,
        capabilities: ['preview'],
        declaredPermissions: ['files.read'],
        grantedPermissions: ['files.read'],
        requestedCommands: [],
        allowedCommands: [],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        contributions: {
          preview: [
            {
              id: 'office',
              title: 'Office 预览',
              extensions: ['docx', 'xlsx', 'pptx'],
              priority: 100,
              panel: {
                entry: 'ui/index.html',
                title: 'Office 预览',
                defaultSize: { width: 720, height: 600 },
              },
            },
          ],
        },
      },
      {
        id: 'org.fiex.sqlite-preview',
        name: 'SQLite 查看器',
        version: '0.2.9',
        description: '只读浏览 SQLite 数据库：表、字段、索引、外键与表数据。',
        enabled: true,
        approved: true,
        backendAvailable: true,
        capabilities: ['preview'],
        declaredPermissions: ['files.read'],
        grantedPermissions: ['files.read'],
        requestedCommands: [],
        allowedCommands: [],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        contributions: {
          preview: [
            {
              id: 'sqlite',
              title: 'SQLite 查看器',
              extensions: ['db', 'sqlite', 'sqlite3'],
              priority: 100,
              panel: {
                entry: 'ui/index.html',
                title: 'SQLite 查看器',
                defaultSize: { width: 720, height: 640 },
              },
            },
          ],
        },
      },
      {
        id: 'org.fiex.data-preview',
        name: '数据预览',
        version: '0.4.0',
        description: '表格数据预览：SQLite、CSV、Excel、Parquet、Arrow、Avro。',
        enabled: true,
        approved: true,
        backendAvailable: true,
        capabilities: ['preview'],
        declaredPermissions: ['files.read'],
        grantedPermissions: ['files.read'],
        requestedCommands: [],
        allowedCommands: [],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        contributions: {
          preview: [
            {
              id: 'data',
              title: '数据预览',
              extensions: [
                'db',
                'sqlite',
                'sqlite3',
                'csv',
                'tsv',
                'xlsx',
                'xls',
                'parquet',
                'arrow',
                'feather',
                'ipc',
                'avro',
              ],
              priority: 110,
              panel: {
                entry: 'ui/index.html',
                title: '数据预览',
                defaultSize: { width: 720, height: 640 },
              },
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

export const demoPlugins: DemoPlugin[] = officialMode
  ? structuredClone(officialPlugins)
  : query.has('pluginFixture')
    ? structuredClone(fixturePlugins)
    : []

export const marketplace = [
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
