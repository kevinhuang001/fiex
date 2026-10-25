import { demoGitEntries, demoGpgKeys, demoHistory } from '../data'
import { demoPlugins, marketplace, officialMode, query } from '../plugins'
import { notHandled } from './shared'

const pluginSettings = new Map<string, unknown>()
const preparedInstalls = new Map<string, (typeof marketplace)[number]>()
const demoMenuSessions = new Map<string, { pluginId: string; actions: Set<string> }>()

export async function handlePlugins(command: string, a: Record<string, unknown>): Promise<unknown> {
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
    case 'plugin_context_menu_resolve': {
      const pluginId = String(a.pluginId || '')
      const plugin = demoPlugins.find((p) => p.id === pluginId)
      if (!plugin?.enabled || plugin.extensions?.contextMenus !== true) {
        throw new Error('Context menu provider is not active')
      }
      const selection = Array.isArray(a.paths)
        ? a.paths.filter((v): v is string => typeof v === 'string')
        : []
      if (selection.length === 0) throw new Error('No selected resources')
      const selectionNames = selection.map((path) => path.split(/[\\/]/).pop() || '')
      const entries = [
        ...(pluginId === 'org.test.fixture'
          ? [
              { id: 'fixture.root', title: '测试插件', kind: 'submenu' },
              { id: 'fixture.panel', title: '信息面板', kind: 'action', parentId: 'fixture.root' },
              { id: 'fixture.action', title: '检查文件', kind: 'action', parentId: 'fixture.root' },
            ]
          : []),
        ...(pluginId === 'org.example.checksum' && a.isDirectory !== true
          ? [{ id: 'checksum.calculate', title: '查看校验和', kind: 'action' }]
          : []),
        ...(pluginId === 'org.fiex.archive'
          ? selection.length === 1 &&
            /\.(zip|rar|7z|tar|gz|tgz|bz2|tbz2|xz|txz)$/i.test(selectionNames[0])
            ? [{ id: 'archive.extract', title: '解压缩', kind: 'action' }]
            : [
                { id: 'archive.compress', title: '压缩', kind: 'submenu' },
                {
                  id: 'archive.advanced',
                  title: '高级压缩…',
                  kind: 'action',
                  parentId: 'archive.compress',
                },
              ]
          : []),
        ...(pluginId === 'org.fiex.gpg'
          ? [
              { id: 'gpg.menu', title: 'GPG', kind: 'submenu' },
              { id: 'gpg.encrypt', title: '加密…', kind: 'action', parentId: 'gpg.menu' },
              { id: 'gpg.decrypt', title: '解密…', kind: 'action', parentId: 'gpg.menu' },
              { id: 'gpg.sign', title: '签名…', kind: 'action', parentId: 'gpg.menu' },
              ...(selection.length === 1
                ? [{ id: 'gpg.verify', title: '验证签名…', kind: 'action', parentId: 'gpg.menu' }]
                : []),
            ]
          : []),
      ]
      const menuSessionId = crypto.randomUUID()
      demoMenuSessions.set(menuSessionId, {
        pluginId,
        actions: new Set(
          entries.filter((entry) => entry.kind === 'action').map((entry) => entry.id),
        ),
      })
      return { menuSessionId, items: entries }
    }
    case 'plugin_context_menu_execute': {
      const menuSessionId = String(a.menuSessionId || '')
      const session = demoMenuSessions.get(menuSessionId)
      demoMenuSessions.delete(menuSessionId)
      const itemId = String(a.itemId || '')
      if (!session || !session.actions.has(itemId)) {
        throw new Error('Invalid or already used menu session')
      }
      const panelId = itemId
      if (
        itemId === 'fixture.panel' ||
        itemId === 'archive.advanced' ||
        itemId.startsWith('gpg.')
      ) {
        const title =
          itemId === 'fixture.panel'
            ? '信息面板'
            : itemId === 'archive.advanced'
              ? '高级压缩'
              : itemId === 'gpg.encrypt'
                ? 'GPG 加密'
                : itemId === 'gpg.decrypt'
                  ? 'GPG 解密'
                  : itemId === 'gpg.sign'
                    ? 'GPG 签名'
                    : '验证签名'
        return {
          kind: 'panel',
          assetPath: 'demo-plugin-panel',
          panelId,
          title,
          size:
            itemId === 'fixture.panel' ? { width: 620, height: 560 } : { width: 520, height: 420 },
        }
      }
      return { ok: true }
    }
    case 'plugin_invoke': {
      const method = String(a.method || '')
      const params = (a.params || {}) as Record<string, unknown>
      if (officialMode) {
        if (method === 'archive.list') {
          return {
            entries: [
              { name: 'Documents/', isDir: true, size: 0 },
              { name: 'Documents/设计稿.png', isDir: false, size: 524288 },
              { name: 'Documents/spec.md', isDir: false, size: 1280 },
              { name: 'Projects/', isDir: true, size: 0 },
              { name: 'Projects/app.ts', isDir: false, size: 8192 },
              { name: 'README.md', isDir: false, size: 2148 },
              { name: '预算明细.csv', isDir: false, size: 4096 },
              { name: '产品截图.png', isDir: false, size: 2411520 },
            ],
            encrypted: false,
            supported: true,
          }
        }
        if (method === 'archive.extract') {
          return '/Users/kevin/Documents'
        }
        if (method === 'data.open' || method === 'data.page') {
          const path = String(params.path || '')
          const name = path.split('/').pop() || '数据'
          if (/\.(db|sqlite|sqlite3)$/i.test(name)) {
            const tables: Record<
              string,
              { columns: { name: string; kind: string }[]; rows: (string | number | null)[][] }
            > = {
              users: {
                columns: [
                  { name: 'id', kind: 'number' },
                  { name: 'name', kind: 'text' },
                  { name: 'email', kind: 'text' },
                  { name: 'created_at', kind: 'text' },
                ],
                rows: [
                  [1, 'Ada Lovelace', 'ada@example.com', '2026-01-04 09:12:00'],
                  [2, 'Grace Hopper', 'grace@example.com', '2026-02-11 14:05:00'],
                  [3, 'Alan Turing', null, '2026-03-02 08:30:00'],
                  [4, 'Katherine Johnson', 'katherine@example.com', null],
                ],
              },
              orders: {
                columns: [
                  { name: 'id', kind: 'number' },
                  { name: 'user_id', kind: 'number' },
                  { name: 'total', kind: 'number' },
                  { name: 'created_at', kind: 'text' },
                ],
                rows: [
                  [1, 1, 128.5, '2026-04-01 10:00:00'],
                  [2, 2, 64, '2026-04-02 11:30:00'],
                  [3, 1, 512.75, '2026-04-03 16:20:00'],
                ],
              },
              active_users: {
                columns: [
                  { name: 'id', kind: 'number' },
                  { name: 'name', kind: 'text' },
                ],
                rows: [
                  [1, 'Ada Lovelace'],
                  [2, 'Grace Hopper'],
                ],
              },
            }
            const datasets = [
              { id: 'users', name: 'users', kind: 'table' },
              { id: 'orders', name: 'orders', kind: 'table' },
              { id: 'active_users', name: 'active_users', kind: 'view' },
            ]
            const table = tables[String(params.dataset || 'users')] || tables.users
            const offset = Number(params.offset || 0)
            const limit = Number(params.limit || 100)
            const page = {
              columns: table.columns,
              rows: table.rows.slice(offset, offset + limit),
              total: table.rows.length,
              offset,
              limit,
            }
            if (method === 'data.open') {
              return {
                format: 'sqlite',
                datasets,
                page,
                summary: {
                  datasets: datasets.length,
                  columns: table.columns.length,
                  rows: table.rows.length,
                },
              }
            }
            return page
          }
          const columns = [
            { name: '部门', kind: 'text' },
            { name: '月份', kind: 'text' },
            { name: '预算', kind: 'number' },
            { name: '实际', kind: 'number' },
            { name: '差异', kind: 'number' },
            { name: '备注', kind: 'text' },
          ]
          const departments = ['研发', '市场', '运营', '设计', '销售']
          const rows = Array.from({ length: 126 }, (_, index) => {
            const budget = 80000 + ((index * 3700) % 60000)
            const actual = budget + (((index * 913) % 9000) - 4500)
            return [
              departments[index % departments.length],
              `2026-${String((index % 12) + 1).padStart(2, '0')}`,
              budget,
              actual,
              actual - budget,
              index % 7 === 0 ? null : index % 5 === 0 ? '季度调整' : '',
            ]
          })
          const order = typeof params.order === 'string' ? params.order : null
          const descending = Boolean(params.descending)
          const sorted = [...rows]
          if (order) {
            const index = columns.findIndex((column) => column.name === order)
            if (index >= 0)
              sorted.sort((left, right) => {
                const a = left[index]
                const b = right[index]
                if (a === null) return 1
                if (b === null) return -1
                const result =
                  typeof a === 'number' && typeof b === 'number'
                    ? a - b
                    : String(a).localeCompare(String(b))
                return descending ? -result : result
              })
          }
          const offset = Number(params.offset || 0)
          const limit = Number(params.limit || 100)
          const content = {
            columns,
            rows: sorted.slice(offset, offset + limit),
            total: sorted.length,
            offset,
            limit,
            order,
            descending,
            note: path.endsWith('.csv') ? '分隔符：逗号' : null,
          }
          if (method === 'data.open')
            return {
              format: (name.split('.').pop() || 'csv').toLowerCase(),
              datasets: [{ id: 'data', name, kind: 'table' }],
              page: content,
            }
          return content
        }
        if (method === 'sqlite.open')
          return {
            tables: [
              { name: 'users', type: 'table', sql: 'CREATE TABLE users (...)' },
              { name: 'orders', type: 'table', sql: 'CREATE TABLE orders (...)' },
              { name: 'active_users', type: 'view', sql: 'CREATE VIEW active_users AS ...' },
            ],
          }
        if (method === 'sqlite.table') {
          const name = String(params.table || '')
          const schema = {
            users: {
              columns: [
                {
                  cid: 0,
                  name: 'id',
                  type: 'INTEGER',
                  notNull: false,
                  default: null,
                  primaryKey: true,
                },
                {
                  cid: 1,
                  name: 'name',
                  type: 'TEXT',
                  notNull: true,
                  default: null,
                  primaryKey: false,
                },
                {
                  cid: 2,
                  name: 'email',
                  type: 'TEXT',
                  notNull: false,
                  default: null,
                  primaryKey: false,
                },
                {
                  cid: 3,
                  name: 'created_at',
                  type: 'TEXT',
                  notNull: false,
                  default: 'CURRENT_TIMESTAMP',
                  primaryKey: false,
                },
              ],
              rows: [
                [1, 'Ada Lovelace', 'ada@example.com', '2026-01-04 09:12:00'],
                [2, 'Grace Hopper', 'grace@example.com', '2026-02-11 14:05:00'],
                [3, 'Alan Turing', null, '2026-03-02 08:30:00'],
                [4, 'Katherine Johnson', 'katherine@example.com', null],
              ],
              indexes: [
                { name: 'sqlite_autoindex_users_1', unique: true, origin: 'u', columns: ['id'] },
                { name: 'users_email', unique: true, origin: 'c', columns: ['email'] },
              ],
              foreignKeys: [],
            },
            orders: {
              columns: [
                {
                  cid: 0,
                  name: 'id',
                  type: 'INTEGER',
                  notNull: false,
                  default: null,
                  primaryKey: true,
                },
                {
                  cid: 1,
                  name: 'user_id',
                  type: 'INTEGER',
                  notNull: true,
                  default: null,
                  primaryKey: false,
                },
                {
                  cid: 2,
                  name: 'total',
                  type: 'REAL',
                  notNull: true,
                  default: '0',
                  primaryKey: false,
                },
                {
                  cid: 3,
                  name: 'status',
                  type: 'TEXT',
                  notNull: false,
                  default: "'paid'",
                  primaryKey: false,
                },
              ],
              rows: [
                [1, 1, 128.5, 'paid'],
                [2, 1, 42, 'paid'],
                [3, 2, 999.99, 'refunded'],
                [4, 3, 7.5, 'pending'],
                [5, 3, 64, 'paid'],
              ],
              indexes: [{ name: 'orders_user', unique: false, origin: 'c', columns: ['user_id'] }],
              foreignKeys: [{ id: 0, table: 'users', from: 'user_id', to: 'id' }],
            },
            active_users: {
              columns: [
                {
                  cid: 0,
                  name: 'id',
                  type: 'INTEGER',
                  notNull: false,
                  default: null,
                  primaryKey: false,
                },
                {
                  cid: 1,
                  name: 'name',
                  type: 'TEXT',
                  notNull: true,
                  default: null,
                  primaryKey: false,
                },
              ],
              rows: [
                [1, 'Ada Lovelace'],
                [2, 'Grace Hopper'],
              ],
              indexes: [],
              foreignKeys: [],
            },
          } as Record<
            string,
            { columns: unknown[]; rows: unknown[][]; indexes: unknown[]; foreignKeys: unknown[] }
          >
          const entry = schema[name] || schema.users
          return {
            columns: entry.columns,
            rows: entry.rows,
            total: entry.rows.length,
            limit: Number(params.limit) || 100,
            offset: Number(params.offset) || 0,
            order: params.order ?? null,
            descending: Boolean(params.descending),
            indexes: entry.indexes,
            foreignKeys: entry.foreignKeys,
          }
        }
        if (method === 'office.render') {
          const path = String(params.path || '')
          const extension = path.slice(path.lastIndexOf('.') + 1).toLowerCase()
          // Real office_oxide output for the bundled demo fixtures.
          const rendered: Record<string, string> = {
            docx: '<h1><span style="font-family:\'Cambria\';font-size:11pt">2026 \u5e74\u7b2c\u4e09\u5b63\u5ea6\u4ea7\u54c1\u62a5\u544a</span></h1>\n<p><span style="font-family:\'Cambria\';font-size:11pt">\u672c\u5b63\u5ea6 </span><span style="font-family:\'Cambria\';font-size:11pt"><strong>\u6d3b\u8dc3\u7528\u6237\u589e\u957f 24%</strong></span><span style="font-family:\'Cambria\';font-size:11pt">\uff0c\u56e2\u961f\u7248\u8f6c\u5316\u7387\u63d0\u5347 6 \u4e2a\u767e\u5206\u70b9\u3002</span></p>\n<h2><span style="font-family:\'Cambria\';font-size:11pt">\u6838\u5fc3\u8fdb\u5c55</span></h2>\n<ul>\n<li><p><span style="font-family:\'Cambria\';font-size:11pt">\u63d2\u4ef6\u5e02\u573a\u5b8c\u6210\u7b7e\u540d\u6821\u9a8c\u4e0e\u589e\u91cf\u66f4\u65b0\uff0c\u5e73\u5747\u5b89\u88c5\u65f6\u95f4\u7f29\u77ed\u5230 3 \u79d2\u3002</span></p></li>\n<li><p><span style="font-family:\'Cambria\';font-size:11pt">Office \u9884\u89c8\u3001SQLite \u67e5\u770b\u5668\u8fdb\u5165\u5b98\u65b9\u63d2\u4ef6\u5217\u8868\u3002</span></p></li>\n</ul>\n<table>\n<tr><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u9879\u76ee</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u9884\u7b97</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u5b9e\u9645</span></p></td></tr>\n<tr><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u7814\u53d1</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">1,200,000</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">1,180,000</span></p></td></tr>\n<tr><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u5e02\u573a</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">450,000</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">470,000</span></p></td></tr>\n</table>\n<h2><span style="font-family:\'Cambria\';font-size:11pt">\u4e0b\u4e00\u6b65</span></h2>\n<p><span style="font-family:\'Cambria\';font-size:11pt">\u5b8c\u5584\u9884\u89c8\u4e0e\u641c\u7d22\uff0c\u4fdd\u6301\u6838\u5fc3\u8f7b\u91cf\u3002</span></p>',
            xlsx: '<h2>\u9884\u7b97</h2>\n<table>\n<tr><th><p>\u9879\u76ee</p></th><th><p>Q1</p></th><th><p>Q2</p></th><th><p>Q3</p></th></tr>\n<tr><td><p>\u7814\u53d1</p></td><td><p>400000</p></td><td><p>400000</p></td><td><p>400000</p></td></tr>\n<tr><td><p>\u5e02\u573a</p></td><td><p>150000</p></td><td><p>150000</p></td><td><p>150000</p></td></tr>\n<tr><td><p>\u5408\u8ba1</p></td><td><p></p></td><td><p></p></td><td><p></p></td></tr>\n</table>\n<hr />\n<h2>\u6536\u5165</h2>\n<table>\n<tr><th><p>\u5b63\u5ea6</p></th><th><p>\u6536\u5165</p></th></tr>\n<tr><td><p>Q1</p></td><td><p>820000</p></td></tr>\n<tr><td><p>Q2</p></td><td><p>960000</p></td></tr>\n<tr><td><p>Q3</p></td><td><p>1120000</p></td></tr>\n</table>',
            pptx: '<h2>Fiex \u4ea7\u54c1\u4ecb\u7ecd</h2>\n<p><span style="font-size:32pt">\u4e00\u4e2a\u66f4\u5feb\u7684\u6587\u4ef6\u7ba1\u7406\u5668</span></p>\n<hr />\n<h2>\u63d2\u4ef6\u5316</h2>\n<ul>\n<li><p><span style="font-size:32pt">Office \u9884\u89c8\u3001SQLite \u67e5\u770b\u5668\u3001\u8fdc\u7a0b\u8fde\u63a5\u90fd\u4ee5\u63d2\u4ef6\u5f62\u5f0f\u5b89\u88c5\u3002</span></p></li>\n</ul>\n<hr />\n<h2>\u4e0b\u4e00\u6b65</h2>\n<ul>\n<li><p><span style="font-size:32pt">\u5b8c\u5584\u9884\u89c8\u4e0e\u641c\u7d22\uff0c\u4fdd\u6301\u6838\u5fc3\u8f7b\u91cf\u3002</span></p></li>\n</ul>',
          }
          return { kind: extension, html: rendered[extension] || '' }
        }
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
                row: { style: { backgroundLight: '#fff9f0', backgroundDark: '#2b2118' } },
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
    case 'plugin_file_dialog': {
      const extensions = Array.isArray(a.extensions) ? a.extensions : []
      if (a.mode === 'openFile' && extensions.includes('png')) {
        return '/demo/background.svg'
      }
      if (a.mode === 'saveFile') return '/Users/kevin/Documents/Archive.zip'
      if (a.mode === 'openFile') return '/Users/kevin/Documents/Archive.zip'
      return '/Users/kevin/Documents'
    }
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

          declaredPermissions: entry.permissions,
          grantedPermissions: entry.permissions,
          requestedNetworkDomains: entry.requestedNetworkDomains,
          allowedNetworkDomains: entry.requestedNetworkDomains,
          author: entry.author,
          extensions: entry.extensions || {},
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
    case 'preview_candidates': {
      const extension = String(a.extension || '').toLowerCase()
      const mime = String(a.mime || '').toLowerCase()
      const isDir = Boolean(a.isDir)
      const textExt = new Set([
        'txt',
        'md',
        'markdown',
        'json',
        'csv',
        'tsv',
        'css',
        'html',
        'htm',
        'xml',
        'yml',
        'yaml',
        'rs',
        'ts',
        'tsx',
        'js',
        'jsx',
        'py',
        'go',
        'java',
        'c',
        'cpp',
        'h',
        'sh',
        'log',
        'toml',
        'ini',
      ])
      const values: Array<{ id: string; provider: string; priority: number }> = []
      if (!isDir && (mime.startsWith('text/') || textExt.has(extension)))
        values.push({ id: 'builtin:text', provider: 'Fiex', priority: 10 })
      if (!isDir && mime.startsWith('image/'))
        values.push({ id: 'builtin:image', provider: 'Fiex', priority: 20 })
      if (!isDir && (mime.startsWith('video/') || mime.startsWith('audio/')))
        values.push({ id: 'builtin:media', provider: 'Fiex', priority: 30 })
      if (!isDir && extension === 'pdf')
        values.push({ id: 'builtin:pdf', provider: 'Fiex', priority: 40 })
      if (values.length === 0)
        values.push({ id: 'builtin:fallback', provider: 'Fiex', priority: 0 })
      for (const plugin of demoPlugins) {
        if (!plugin.enabled) continue
        const previews = Array.isArray(plugin.extensions?.preview)
          ? (plugin.extensions.preview as Array<{
              id: string
              title?: string
              extensions?: string[]
              priority?: number
            }>)
          : []
        for (const contribution of previews) {
          const extensions = Array.isArray(contribution.extensions)
            ? contribution.extensions.map((value: string) => value.toLowerCase())
            : []
          if (!extensions.includes(extension)) continue
          values.push({
            id: `plugin:${plugin.id}:${contribution.id}`,
            provider: plugin.name,
            priority: contribution.priority ?? 50,
          })
        }
      }
      return values.sort((left, right) => right.priority - left.priority)
    }
    case 'preview_open': {
      const path = String(a.path || '')
      const name = path.split(/[\\/]/).pop() || ''
      const extension = name.includes('.') ? name.split('.').pop()?.toLowerCase() || '' : ''
      const providerId = String(a.providerId || '')
      const candidates = (await handlePlugins('preview_candidates', {
        extension,
        mime: '',
        isDir: false,
      })) as Array<{ id: string; provider: string; priority: number }>
      const provider = candidates.find((value) => value.id === providerId) || candidates[0]
      if (!provider) throw new Error('没有可用的预览方式')
      if (provider.id.startsWith('plugin:')) {
        const [, pluginId, contributionId] = provider.id.split(':')
        const slug = pluginId.split('.').pop() || ''
        const entry = pluginId === 'org.fiex.archive' ? 'preview.html' : 'index.html'
        const response = await fetch(`/__fiex_official_plugin/${slug}/${entry}`)
        if (!response.ok) throw new Error(await response.text())
        const html = await response.text()
        const sessionId = crypto.randomUUID()
        return {
          providerId: provider.id,
          provider: provider.provider,
          priority: provider.priority,
          view: {
            kind: 'html',
            html,
            sessionId,
            pluginId,
            contributionId,
            context: {
              version: 1,
              sessionId,
              intent: 'preview',
              resource: {
                ref: { mountId: 'local', nodeId: `fiex-resource:${sessionId}` },
                metadata: { name, kind: 'file', extension, size: null },
                read: { resourceId: `fiex-read:${sessionId}` },
              },
            },
          },
        }
      }
      if (provider.id === 'builtin:text') {
        const text = name.endsWith('.json')
          ? '{\n  "name": "Fiex",\n  "preview": "demo",\n  "theme": "light"\n}'
          : '# Fiex\n\n留一点空间，给重要的文件。\n\n支持四种视图、快速查找与空格预览。'
        return {
          providerId: provider.id,
          provider: provider.provider,
          priority: provider.priority,
          view: { kind: 'text', text, truncated: false },
        }
      }
      if (provider.id === 'builtin:pdf') {
        return {
          providerId: provider.id,
          provider: provider.provider,
          priority: provider.priority,
          view: { kind: 'pdf', path: '/demo/sample.pdf', name },
        }
      }
      return {
        providerId: provider.id,
        provider: provider.provider,
        priority: provider.priority,
        view: {
          kind: 'fallback',
          name,
          extension,
          message: 'demo 未内置该类型的媒体样本',
        },
      }
    }
    case 'connection_open': {
      if (String(a.id || '') === 'demo-ssh') {
        return { kind: 'terminal', connectionId: 'demo-ssh' }
      }
      // The browser fixture has one WebDAV connection and it registers a VFS,
      // so Core's two-branch navigation resolves to the remote Explorer root.
      if (String(a.id || '') === 'demo-webdav') {
        return { kind: 'vfs', path: 'fiex://mount/demo-webdav/' }
      }
      throw new Error('Unknown demo connection')
    }

    default:
      return notHandled
  }
}
