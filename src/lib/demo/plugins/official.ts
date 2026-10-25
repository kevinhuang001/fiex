import type { DemoPlugin } from './types'

export const query = new URLSearchParams(location.search)
export const officialMode =
  query.has('officialPlugins') ||
  (import.meta.env.DEV && !query.has('demo') && !query.has('pluginFixture'))
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
        extensions: {
          vfs: { title: 'Git 历史', readOnly: true },
          toolbar: {
            title: '源代码管理',
            icon: 'git-branch',
            panel: {
              entry: 'ui/dist/index.html',
              title: '源代码管理',
              defaultSize: { width: 480, height: 300 },
            },
          },
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
        extensions: {
          contextMenus: true,
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
          info: {
            extensions: ['zip', '7z', 'rar', 'tar', 'gz', 'tgz', 'bz2', 'tbz2', 'xz', 'txz'],
            priority: 100,
          },
          preview: {
            extensions: ['zip', '7z', 'rar', 'tar', 'gz', 'tgz', 'bz2', 'tbz2', 'xz', 'txz'],
            priority: 100,
          },
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

        declaredPermissions: ['files.read'],
        grantedPermissions: ['files.read'],
        requestedCommands: [],
        allowedCommands: [],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        extensions: {
          info: { command: 'office.info', extensions: ['docx', 'xlsx', 'pptx'], priority: 100 },
          preview: {
            extensions: ['docx', 'xlsx', 'pptx'],
            priority: 100,
            panel: {
              entry: 'ui/index.html',
              defaultSize: { width: 720, height: 600 },
            },
          },
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

        declaredPermissions: ['files.read'],
        grantedPermissions: ['files.read'],
        requestedCommands: [],
        allowedCommands: [],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        extensions: {
          preview: {
            extensions: ['db', 'sqlite', 'sqlite3'],
            priority: 100,
            panel: {
              entry: 'ui/index.html',
              defaultSize: { width: 720, height: 640 },
            },
          },
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

        declaredPermissions: ['files.read'],
        grantedPermissions: ['files.read'],
        requestedCommands: [],
        allowedCommands: [],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        extensions: {
          info: {
            command: 'data.info',
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
            priority: 100,
          },
          preview: {
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
              defaultSize: { width: 720, height: 640 },
            },
          },
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

        declaredPermissions: ['commands'],
        grantedPermissions: ['commands'],
        requestedCommands: ['gpg'],
        allowedCommands: ['gpg'],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        extensions: {
          toolbar: {
            title: '公钥管理',
            icon: 'key-round',
            panel: {
              entry: 'ui/dist/index.html',
              title: 'GPG 公钥管理',
              defaultSize: { width: 420, height: 460 },
            },
          },
          contextMenus: true,
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

        declaredPermissions: ['folders.read', 'files.read'],
        grantedPermissions: ['folders.read', 'files.read'],
        requestedNetworkDomains: [],
        allowedNetworkDomains: [],
        author: { name: 'Fiex Contributors' },
        extensions: {
          info: { command: 'checksum.info', extensions: [], priority: 100 },
        },
      },
    ]
  : []
