import type { Listing, OperationProgress } from '@/lib/types'
import { basename, join, parent } from '@/lib/files'
import {
  fixtureChanges,
  fixtureRemoteEntries,
  fixtureRemoteRoot,
  fixtureRemoteWarnings,
  ignored,
  item,
  root,
} from '../data'
import { notHandled, state } from './shared'

type FixtureInfoField = {
  id: string
  sectionId: string
  label: string
  compute: 'onOpen' | 'onDemand'
  status: 'idle' | 'ready' | 'pending' | 'unavailable' | 'error'
  value?: string
  error?: string
}
type FixtureInfoSection = {
  id: string
  title: string
  providerId: string
  provider: string
  source: 'core' | 'plugin'
  pluginId?: string
  fields: FixtureInfoField[]
  error?: string
}
const infoSessions = new Map<string, { path: string; sections: FixtureInfoSection[] }>()

function infoField(
  sectionId: string,
  id: string,
  label: string,
  value: string | undefined,
  compute: 'onOpen' | 'onDemand' = 'onOpen',
): FixtureInfoField {
  return {
    id,
    sectionId,
    label,
    compute,
    status: value === undefined ? 'idle' : 'ready',
    ...(value === undefined ? {} : { value }),
  }
}

function infoSection(
  providerId: string,
  provider: string,
  title: string,
  sectionId: string,
  fields: FixtureInfoField[],
  pluginId?: string,
): FixtureInfoSection {
  return {
    id: `${providerId}::${sectionId}`,
    title,
    providerId,
    provider,
    source: pluginId ? 'plugin' : 'core',
    ...(pluginId ? { pluginId } : {}),
    fields,
  }
}

function fixtureInfoSections(entry: (typeof state.entries)[number]): FixtureInfoSection[] {
  const ext = entry.name.split('.').pop()?.toLowerCase() || ''
  const basic = infoSection('core.metadata', 'Fiex', '基本信息', 'basic', [
    infoField('basic', 'name', '名称', entry.name),
    infoField(
      'basic',
      'kind',
      '种类',
      entry.isDir ? '文件夹' : entry.isSymlink ? '符号链接' : '文件',
    ),
    infoField('basic', 'location', '位置', parent(entry.path)),
    ...(!entry.isDir && ext ? [infoField('basic', 'extension', '扩展名', ext)] : []),
    ...(!entry.isDir && entry.mime ? [infoField('basic', 'mime', 'MIME 类型', entry.mime)] : []),
    ...(!entry.isDir
      ? [infoField('basic', 'size', '大小', `${entry.size} B (${entry.size} bytes)`)]
      : []),
  ])
  const sections = [basic]
  sections.push(
    infoSection('core.metadata', 'Fiex', '时间', 'times', [
      infoField('times', 'created', '创建时间', new Date(entry.created).toLocaleString()),
      infoField('times', 'modified', '修改时间', new Date(entry.modified).toLocaleString()),
    ]),
  )
  if (entry.isDir)
    sections.push(
      infoSection('core.metadata', 'Fiex', '目录统计', 'directoryStats', [
        infoField('directoryStats', 'directFiles', '直接文件数', undefined, 'onDemand'),
        infoField('directoryStats', 'recursiveFiles', '递归文件总数', undefined, 'onDemand'),
        infoField('directoryStats', 'recursiveSize', '递归文件总大小', undefined, 'onDemand'),
      ]),
    )
  if (!entry.isDir) {
    if (['zip', '7z', 'rar', 'tar', 'gz', 'tgz'].includes(ext))
      sections.push(
        infoSection(
          'org.fiex.archive',
          '压缩与解压',
          '压缩包',
          'archive',
          [
            infoField('archive', 'format', '格式', undefined, 'onDemand'),
            infoField('archive', 'compressedSize', '压缩后大小', undefined, 'onDemand'),
            infoField('archive', 'uncompressedSize', '解压后大小', undefined, 'onDemand'),
            infoField('archive', 'entryCount', '文件数', undefined, 'onDemand'),
            infoField('archive', 'compressionRatio', '压缩率', undefined, 'onDemand'),
          ],
          'org.fiex.archive',
        ),
      )
    if (['docx', 'xlsx', 'pptx'].includes(ext))
      sections.push(
        infoSection(
          'org.fiex.office-preview',
          'Office 预览',
          '文档',
          'office',
          [
            infoField('office', 'format', '格式', undefined, 'onOpen'),
            infoField('office', 'paragraphs', '段落', undefined, 'onDemand'),
            infoField('office', 'tables', '表格', undefined, 'onDemand'),
            infoField('office', 'images', '图片', undefined, 'onDemand'),
          ],
          'org.fiex.office-preview',
        ),
      )
    if (['db', 'sqlite', 'sqlite3', 'csv', 'tsv', 'parquet', 'arrow', 'avro'].includes(ext))
      sections.push(
        infoSection(
          'org.fiex.data-preview',
          '数据预览',
          '数据表',
          'data',
          [
            infoField('data', 'format', '格式', undefined, 'onOpen'),
            infoField('data', 'datasets', '数据集', undefined, 'onDemand'),
            infoField('data', 'columns', '首表列数', undefined, 'onDemand'),
            infoField('data', 'rows', '首表行数', undefined, 'onDemand'),
          ],
          'org.fiex.data-preview',
        ),
      )
    sections.push(
      infoSection(
        'org.fiex.checksum',
        '校验和',
        '校验和',
        'checksum',
        [
          ...[
            ['sha256', 'SHA-256'],
            ['sha512', 'SHA-512'],
            ['sha1', 'SHA-1'],
            ['md5', 'MD5'],
            ['crc32', 'CRC32'],
          ].map(([id, label]) => infoField('checksum', id!, label!, undefined, 'onDemand')),
        ],
        'org.fiex.checksum',
      ),
    )
  }
  return sections
}

function computeFixtureField(field: FixtureInfoField, entry: (typeof state.entries)[number]) {
  const ext = entry.name.split('.').pop()?.toUpperCase() || ''
  const values: Record<string, string> = {
    sha256: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    sha512: 'ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a',
    sha1: 'a9993e364706816aba3e25717850c26c9cd0d89d',
    md5: '900150983cd24fb0d6963f7d28e17f72',
    crc32: '352441c2',
    format: ext,
    compressedSize: `${entry.size} B`,
    uncompressedSize: `${Math.round(entry.size * 2.4)} B`,
    entryCount: '18',
    compressionRatio: '58.3%',
    paragraphs: '42',
    tables: ext === 'XLSX' ? '3' : '1',
    images: '2',
    datasets: '2',
    columns: '6',
    rows: '128',
    directFiles: '4',
    recursiveFiles: '18',
    recursiveSize: '4.2 MiB',
  }
  field.value = values[field.id]
  field.status = field.value === undefined ? 'unavailable' : 'ready'
}

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
            allowedOps: entry.isDir ? ['stat', 'list', 'search', 'remove'] : ['stat', 'remove'],
          }))
        return {
          path: 'trash://',
          entries,
          allowedOps: ['list', 'stat', 'search'],
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
          allowedOps: ['stat'],
          linkTarget: null,
          tags: [],
          hasCache: false,
          pinned: false,
          uploaded: false,
          sync: null,
        })
        return {
          path: 'applications://',
          allowedOps: ['list', 'stat', 'search', 'write', 'mkdir'],
          virtualVolume: true,
          defaultView: 'icons',
          warnings: [],
          entries: [
            app('Fiex', 'applications://app_fiex', 'app'),
            app('我的快捷方式', 'applications://app_my-app', 'app'),
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
          allowedOps: ['list', 'stat', 'search'],
          entries: names
            .map((name) => state.entries.find((entry) => entry.name === name))
            .filter((entry): entry is NonNullable<typeof entry> => Boolean(entry)),
          warnings: [],
        }
      }
      if ((a.path as string).startsWith(fixtureRemoteRoot)) {
        return {
          path: a.path,
          allowedOps: ['list', 'stat', 'search', 'write', 'mkdir', 'rename', 'remove'],
          entries: fixtureRemoteEntries,
          warnings: fixtureRemoteWarnings,
        }
      }

      if (
        !state.entries.some((e) => e.path === a.path && e.isDir) &&
        a.path !== root &&
        a.path !== '/'
      )
        throw new Error('文件夹不存在')
      return {
        path: a.path as string,
        allowedOps: ['list', 'stat', 'search', 'write', 'mkdir', 'rename', 'remove'],
        entries: state.entries.filter(
          (e) => parent(e.path) === a.path && (Boolean(a.hidden) || !e.name.startsWith('.')),
        ),
        warnings: [],
      } satisfies Listing
    }
    case 'file_info':
      return state.entries.find((e) => e.path === a.path)
    case 'file_parent':
      return parent(String(a.path ?? ''))
    case 'file_info_open': {
      const entry = state.entries.find((e) => e.path === a.path)
      if (!entry) throw new Error('文件不存在')
      const sessionId = `fixture-info-${crypto.randomUUID()}`
      const sections = fixtureInfoSections(entry)
      infoSessions.set(sessionId, { path: entry.path, sections })
      for (const field of sections.flatMap((section) => section.fields)) {
        if (field.compute === 'onOpen') computeFixtureField(field, entry)
      }
      return { sessionId, sections }
    }
    case 'file_info_compute': {
      const session = infoSessions.get(String(a.sessionId))
      if (!session) throw new Error('Info session expired')
      const requests = a.fields as { sectionId: string; fieldId: string }[]
      const entry = state.entries.find((item) => item.path === session.path)
      if (!entry) throw new Error('Info resource no longer exists')
      const updates = []
      for (const request of requests) {
        const section = session.sections.find(
          (item) =>
            item.providerId === a.providerId &&
            item.fields.some(
              (candidate) =>
                candidate.sectionId === request.sectionId && candidate.id === request.fieldId,
            ),
        )
        const field = section?.fields.find(
          (item) => item.sectionId === request.sectionId && item.id === request.fieldId,
        )
        if (!field) throw new Error('Info field is not declared')
        computeFixtureField(field, entry)
        updates.push({
          providerId: String(a.providerId),
          sectionId: field.sectionId,
          fieldId: field.id,
          status: field.status,
          value: field.value,
          error: field.error,
        })
      }
      return { sessionId: String(a.sessionId), revision: 1, updates }
    }
    case 'file_info_cancel':
      return null
    case 'file_info_session_close':
      infoSessions.delete(String(a.sessionId))
      return null
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
      if (fixtureChanges[previous]) {
        fixtureChanges[path] = fixtureChanges[previous]
        delete fixtureChanges[previous]
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
