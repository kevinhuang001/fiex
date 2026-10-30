import type { Listing, OperationProgress } from '../../types'
import { basename, join, parent } from '../../files'
import {
  demoChanges,
  demoRemoteEntries,
  demoRemoteRoot,
  demoRemoteWarnings,
  ignored,
  item,
  root,
} from '../data'
import { notHandled, state } from './shared'

export async function handleFiles(command: string, a: Record<string, unknown>): Promise<unknown> {
  switch (command) {
    case 'list_directory': {
      const requested = String(a.path || '')
      if (requested === 'trash://') {
        // Trash entries use opaque identities, just like Core: the backing
        // host path never reaches the UI.
        const trashId = (name: string) =>
          `trash://${btoa(name).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')}`
        const entries = state.entries
          .filter((entry) => parent(entry.path) === `${root}/.Trash`)
          .map((entry) => ({
            ...entry,
            path: trashId(entry.name),
            readonly: true,
            allowedOps: ['stat'],
          }))
        return {
          path: 'trash://',
          entries,
          readOnly: true,
          canCreate: false,
          virtualVolume: true,
          warnings: [],
        }
      }
      if (requested === 'applications://') {
        const app = (name: string, path: string, extension: string) => ({
          path,
          name,
          isDir: false,
          isSymlink: false,
          size: 0,
          modified: 0,
          created: 0,
          extension,
          mime: 'application/x-fiex-application',
          readonly: false,
          linkTarget: null,
          tags: [],
          hasCache: false,
          pinned: false,
          uploaded: false,
          sync: null,
        })
        return {
          path: 'applications://',
          readOnly: false,
          canCreate: false,
          virtualVolume: true,
          defaultView: 'icons',
          warnings: [],
          entries: [
            app('Fiex', 'applications://ZmlleHQ=', 'app'),
            app('我的快捷方式', 'applications://user/bXktYXBw', 'app'),
          ],
        }
      }
      if (requested === 'recent:' || requested === 'recent://') {
        const names = [
          'Notes.md',
          '产品介绍.pptx',
          '预算明细.csv',
          '示例数据库.db',
          'Palette.json',
          'Archive.zip',
        ]
        return {
          path: 'recent://',
          entries: names
            .map((name) => state.entries.find((entry) => entry.name === name))
            .filter((entry): entry is NonNullable<typeof entry> => Boolean(entry)),
          warnings: [],
        }
      }
      if ((a.path as string).startsWith(demoRemoteRoot)) {
        return { path: a.path, entries: demoRemoteEntries, warnings: demoRemoteWarnings }
      }

      if (
        !state.entries.some((e) => e.path === a.path && e.isDir) &&
        a.path !== root &&
        a.path !== '/'
      )
        throw new Error('文件夹不存在')
      return {
        path: a.path as string,
        entries: state.entries.filter(
          (e) => parent(e.path) === a.path && (Boolean(a.hidden) || !e.name.startsWith('.')),
        ),
        warnings: [],
      } satisfies Listing
    }
    case 'file_info':
      return state.entries.find((e) => e.path === a.path)
    case 'file_info_details': {
      const entry = state.entries.find((e) => e.path === a.path)
      if (!entry) return []
      const ext = entry.name.split('.').pop()?.toLowerCase() || ''
      const sections = []
      if (!entry.isDir) {
        sections.push({
          id: 'org.fiex.checksum.checksum',
          source: 'plugin',
          title: '校验和',
          pluginId: 'org.fiex.checksum',
          provider: '校验和',
          rows: [
            {
              label: 'SHA-256',
              value: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
              copyable: true,
            },
            {
              label: 'SHA-512',
              value: 'ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a',
              copyable: true,
            },
            { label: 'SHA-1', value: 'a9993e364706816aba3e25717850c26c9cd0d89d', copyable: true },
            { label: 'MD5', value: '900150983cd24fb0d6963f7d28e17f72', copyable: true },
            { label: 'CRC32', value: '352441c2', copyable: true },
          ],
        })
      }
      if (['zip', '7z', 'rar', 'tar', 'gz', 'tgz'].includes(ext)) {
        sections.push({
          id: 'org.fiex.archive.archive',
          source: 'plugin',
          title: '压缩包',
          pluginId: 'org.fiex.archive',
          provider: '压缩与解压',
          rows: [
            { label: '格式', value: ext.toUpperCase() },
            { label: '压缩后大小', value: `${entry.size} B` },
            { label: '解压后大小', value: `${Math.round(entry.size * 2.4)} B` },
            { label: '文件数', value: '18' },
            { label: '压缩率', value: '58.3%' },
          ],
        })
      }
      if (['docx', 'xlsx', 'pptx'].includes(ext)) {
        sections.push({
          id: 'org.fiex.office-preview.office',
          source: 'plugin',
          title: '文档',
          pluginId: 'org.fiex.office-preview',
          provider: 'Office 预览',
          rows: [
            { label: '格式', value: ext.toUpperCase() },
            { label: '段落', value: '42' },
            { label: '表格', value: ext === 'xlsx' ? '3' : '1' },
            { label: '图片', value: '2' },
          ],
        })
      }
      if (['db', 'sqlite', 'sqlite3', 'csv', 'tsv', 'parquet', 'arrow', 'avro'].includes(ext)) {
        sections.push({
          id: 'org.fiex.data-preview.data',
          source: 'plugin',
          title: '数据表',
          pluginId: 'org.fiex.data-preview',
          provider: '数据预览',
          rows: [
            { label: '格式', value: ext.toUpperCase() },
            { label: '数据集', value: '2' },
            { label: '首表列数', value: '6' },
            { label: '首表行数', value: '128' },
          ],
        })
      }
      return sections
    }
    case 'read_preview':
      if (String(a.path).endsWith('/Palette.json'))
        return '{\n  "accent": "#387ca8",\n  "surface": "#f8fafc",\n  "radius": 12\n}'
      return '# Fiex\n\n留一点空间，给重要的文件。\n\n支持四种视图、快速查找与空格预览。'
    case 'set_tags': {
      const tags = a.tags as { name: string; color: number }[]
      state.entries = state.entries.map((entry) =>
        (a.paths as string[]).includes(entry.path) ? { ...entry, tags } : entry,
      )
      return null
    }
    case 'search_files': {
      const providers = String(a.query).toLowerCase().includes('plugin-index-hit')
        ? [item('/Users/kevin/Documents/Indexed result.txt', false)]
        : []
      return {
        entries: [
          ...state.entries.filter(
            (e) =>
              e.path.startsWith(a.root as string) &&
              e.name.toLowerCase().includes((a.query as string).toLowerCase()) &&
              (a.hidden || !e.name.startsWith('.')) &&
              !ignored(e.name, a.ignores),
          ),
          ...providers.filter(
            (e) =>
              e.path.startsWith(String(a.root).replace(/\/$/, '') + '/') &&
              !ignored(e.name, a.ignores),
          ),
        ],
        done: true,
        elapsedMs: 12,
        truncated: false,
        skipped: 0,
      }
    }
    case 'create_entry': {
      const path = join(a.path as string, a.name as string)
      if (state.entries.some((e) => e.path === path)) throw new Error('同名项目已存在')
      state.entries.push(item(path, true))
      return path
    }
    case 'rename_entry': {
      const path = join(parent(a.path as string), a.name as string)
      state.entries = state.entries.map((e) =>
        e.path === a.path ? { ...e, path, name: a.name as string } : e,
      )
      const previous = String(a.path)
      if (demoChanges[previous]) {
        demoChanges[path] = demoChanges[previous]
        delete demoChanges[previous]
      }
      return path
    }
    case 'recycle':
      state.entries = state.entries.filter((e) => !(a.paths as string[]).includes(e.path))
      return { paths: a.paths, errors: [], skipped: 0 }
    case 'remote_download':
    case 'remote_open':
    case 'remote_cache': {
      const progress = a.onProgress as { onmessage?: (v: OperationProgress) => void } | undefined
      const total = 8 * 1024 * 1024
      const step = basename(String(a.path || (a.paths as string[])?.[0] || ''))
      for (let index = 1; progress?.onmessage && index <= 6; index += 1) {
        progress.onmessage({ done: Math.round((total * index) / 6), total, step, unit: 'bytes' })
        await new Promise((resolve) => setTimeout(resolve, 140))
      }
      return null
    }
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
        const incoming = state.entries.find((e) => e.path === source)
        const existing = state.entries.find((e) => e.path === destination)
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
            if (!state.entries.some((e) => e.path === candidate)) {
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
        const selected = state.entries.filter(
          (e) => e.path === source || e.path.startsWith(source + '/'),
        )
        const transferred = selected.map((e) => ({
          ...e,
          path: destination + e.path.slice(source.length),
          name: e.path === source ? basename(destination) : e.name,
        }))
        if (action === 'replace')
          state.entries = state.entries.filter(
            (e) => e.path !== destination && !e.path.startsWith(destination + '/'),
          )
        if (a.moving) state.entries = state.entries.filter((e) => !selected.includes(e))
        state.entries.push(...transferred)
        paths.push(destination)
      }
      return { paths, errors: [], skipped: 0, conflicts: [] }
    }

    default:
      return notHandled
  }
}
