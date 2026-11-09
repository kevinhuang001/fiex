import { fixtureGitEntries, fixtureGpgKeys, fixtureHistory } from '../../data'
import { officialMode } from '../../plugins'
import { createFixtureGpgKey, deleteFixtureGpgKey } from '../../gpgKeys'
import { notHandled } from '../shared'
import { fixturePreviewNames, pluginSettings } from './state'

export async function handlePluginInvoke(
  command: string,
  a: Record<string, unknown>,
): Promise<unknown> {
  switch (command) {
    case 'plugin_invoke': {
      const method = String(a.method || '')
      const params = { ...((a.params || {}) as Record<string, unknown>) }
      if (!params.path && a.previewSessionId) {
        const name = fixturePreviewNames.get(String(a.previewSessionId))
        if (name) params.path = name
      }
      if (officialMode) {
        if (method === 'archive.list') {
          return {
            entries: [
              { name: 'Documents/', isDir: true, size: 0 },
              { name: '空文件夹/', isDir: true, size: 0 },
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
        if (method === 'office.render') {
          const path = String(params.path || '')
          const extension = path.slice(path.lastIndexOf('.') + 1).toLowerCase()
          // Real office_oxide output for the bundled demo fixtures.
          const rendered: Record<string, string> = {
            docx: '<h1><span style="font-family:\'Cambria\';font-size:11pt">2026 \u5e74\u7b2c\u4e09\u5b63\u5ea6\u4ea7\u54c1\u62a5\u544a</span></h1>\n<p><span style="font-family:\'Cambria\';font-size:11pt">\u672c\u5b63\u5ea6 </span><span style="font-family:\'Cambria\';font-size:11pt"><strong>\u6d3b\u8dc3\u7528\u6237\u589e\u957f 24%</strong></span><span style="font-family:\'Cambria\';font-size:11pt">\uff0c\u56e2\u961f\u7248\u8f6c\u5316\u7387\u63d0\u5347 6 \u4e2a\u767e\u5206\u70b9\u3002</span></p>\n<h2><span style="font-family:\'Cambria\';font-size:11pt">\u6838\u5fc3\u8fdb\u5c55</span></h2>\n<ul>\n<li><p><span style="font-family:\'Cambria\';font-size:11pt">\u63d2\u4ef6\u5e02\u573a\u5b8c\u6210\u7b7e\u540d\u6821\u9a8c\u4e0e\u589e\u91cf\u66f4\u65b0\uff0c\u5e73\u5747\u5b89\u88c5\u65f6\u95f4\u7f29\u77ed\u5230 3 \u79d2\u3002</span></p></li>\n<li><p><span style="font-family:\'Cambria\';font-size:11pt">Office \u9884\u89c8\u3001SQLite \u67e5\u770b\u5668\u8fdb\u5165\u5b98\u65b9\u63d2\u4ef6\u5217\u8868\u3002</span></p></li>\n</ul>\n<table>\n<tr><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u9879\u76ee</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u9884\u7b97</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u5b9e\u9645</span></p></td></tr>\n<tr><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u7814\u53d1</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">1,200,000</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">1,180,000</span></p></td></tr>\n<tr><td><p><span style="font-family:\'Cambria\';font-size:11pt">\u5e02\u573a</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">450,000</span></p></td><td><p><span style="font-family:\'Cambria\';font-size:11pt">470,000</span></p></td></tr>\n</table>\n<h2><span style="font-family:\'Cambria\';font-size:11pt">\u4e0b\u4e00\u6b65</span></h2>\n<p><span style="font-family:\'Cambria\';font-size:11pt">\u5b8c\u5584\u9884\u89c8\u4e0e\u641c\u7d22\uff0c\u4fdd\u6301\u6838\u5fc3\u8f7b\u91cf\u3002</span></p>',
            xlsx: '<h2>\u9884\u7b97</h2>\n<table>\n<tr><th><p>\u9879\u76ee</p></th><th><p>Q1</p></th><th><p>Q2</p></th><th><p>Q3</p></th></tr>\n<tr><td><p>\u7814\u53d1</p></td><td><p>400000</p></td><td><p>400000</p></td><td><p>400000</p></td></tr>\n<tr><td><p>\u5e02\u573a</p></td><td><p>150000</p></td><td><p>150000</p></td><td><p>150000</p></td></tr>\n<tr><td><p>\u5408\u8ba1</p></td><td><p></p></td><td><p></p></td><td><p></p></td></tr>\n</table>\n<hr />\n<h2>\u6536\u5165</h2>\n<table>\n<tr><th><p>\u5b63\u5ea6</p></th><th><p>\u6536\u5165</p></th></tr>\n<tr><td><p>Q1</p></td><td><p>820000</p></td></tr>\n<tr><td><p>Q2</p></td><td><p>960000</p></td></tr>\n<tr><td><p>Q3</p></td><td><p>1120000</p></td></tr>\n</table>',
            pptx: '<h2>Fiex \u4ea7\u54c1\u4ecb\u7ecd</h2>\n<p><span style="font-size:32pt">\u4e00\u4e2a\u66f4\u5feb\u7684\u6587\u4ef6\u7ba1\u7406\u5668</span></p>\n<hr />\n<h2>\u63d2\u4ef6\u5316</h2>\n<ul>\n<li><p><span style="font-size:32pt">Office \u9884\u89c8\u3001SQLite \u67e5\u770b\u5668\u3001\u8fdc\u7a0b\u8fde\u63a5\u90fd\u4ee5\u63d2\u4ef6\u5f62\u5f0f\u5b89\u88c5\u3002</span></p></li>\n</ul>\n<hr />\n<h2>\u4e0b\u4e00\u6b65</h2>\n<ul>\n<li><p><span style="font-size:32pt">\u5b8c\u5584\u9884\u89c8\u4e0e\u641c\u7d22\uff0c\u4fdd\u6301\u6838\u5fc3\u8f7b\u91cf\u3002</span></p></li>\n</ul>',
          }
          const configured = Number(
            pluginSettings.get('org.fiex.office-preview:global:previewPages') ?? 6,
          )
          const pages = Number.isFinite(configured)
            ? Math.min(30, Math.max(1, Math.trunc(configured)))
            : 6
          let html = rendered[extension] || ''
          let truncated = false
          if (extension === 'pptx' || extension === 'xlsx') {
            const sections = html.split(/\n?<hr \/>\n?/)
            if (sections.length > pages) {
              html = sections.slice(0, pages).join('\n<hr />\n')
              truncated = true
            }
          }
          return { kind: extension, html, pages, truncated }
        }
        if (method === 'git.status')
          return {
            root: '/Users/kevin/Documents',
            branch: 'main',
            entries: { ...fixtureGitEntries },
          }
        if (method === 'git.refs')
          return { head: 'bbbbbbbb', current: 'main', branches: ['main'], tags: [] }
        if (method === 'git.history') return fixtureHistory
        if (method === 'gpg.info') return { version: 'gpg (GnuPG) 2.4.5' }
        if (method === 'gpg.createKey') return createFixtureGpgKey(params)
        if (method === 'gpg.deleteKey') return deleteFixtureGpgKey(params)
        if (method === 'gpg.keys') return fixtureGpgKeys
        if (method === 'gpg.verify')
          return {
            valid: true,
            signer: fixtureGpgKeys[0].name,
            fingerprint: fixtureGpgKeys[0].fingerprint,
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
    default:
      return notHandled
  }
}
