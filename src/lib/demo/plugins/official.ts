import type { DemoPlugin } from './types'

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
          info: [
            {
              id: 'archive.info',
              title: '压缩包',
              command: 'archive.info',
              extensions: ['zip', '7z', 'rar', 'tar', 'gz', 'tgz', 'bz2', 'tbz2', 'xz', 'txz'],
              priority: 100,
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
          info: [
            {
              id: 'office.info',
              title: '文档',
              command: 'office.info',
              extensions: ['docx', 'xlsx', 'pptx'],
              priority: 100,
            },
          ],
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
          info: [
            {
              id: 'data.info',
              title: '数据表',
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
          ],
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
          info: [
            {
              id: 'checksum.info',
              title: '校验和',
              command: 'checksum.info',
              extensions: [],
              priority: 100,
            },
          ],
        },
      },
    ]
  : []
