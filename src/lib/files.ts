import { t, getLocale } from '@/i18n'
import type { Entry, Group, Sort } from './types'
export type CorePath = { root: string; parts: string[] }
export type VirtualRoot = 'recent://' | 'applications://' | 'trash://' | 'recyclebin://'
export type VirtualAddress = { root: VirtualRoot; payload: string }
const VIRTUAL_LABELS: Record<string, VirtualRoot> = {
  最近使用: 'recent://',
  应用程序: 'applications://',
  应用: 'applications://',
  废纸篓: 'trash://',
  回收站: 'recyclebin://',
}
function encodeBase64Url(value: string): string {
  const bytes = new TextEncoder().encode(value)
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}
function decodeBase64Url(value: string): string | null {
  try {
    const base = value.replace(/-/g, '+').replace(/_/g, '/')
    const binary = atob(base + '='.repeat((4 - (base.length % 4)) % 4))
    return new TextDecoder().decode(Uint8Array.from(binary, (char) => char.charCodeAt(0)))
  } catch {
    return null
  }
}
/** Decoded value of an opaque payload, or null when the text is a literal
 *  (Chinese or otherwise) user-entered path that should be encoded. */
function opaquePayload(value: string): string | null {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) return null
  const decoded = decodeBase64Url(value)
  if (!decoded || decoded.includes('\uFFFD') || /[\u0000-\u001f\u007f]/.test(decoded)) return null
  return decoded
}
/** Recognize the Core-owned virtual views without treating Windows drives or
 *  `fiex://mount/...` identities as virtual application paths. */
export function virtualPath(path: string): VirtualAddress | null {
  const match = /^(recent|applications|trash|recyclebin):\/\/(.*)$/i.exec(path)
  if (!match) return null
  const root = `${match[1].toLowerCase()}://` as VirtualRoot
  let payload = match[2]
  if (root === 'applications://') payload = payload.replace(/^~/, '').replace(/^user\//, '')
  return { root, payload }
}
function virtualHref(root: VirtualRoot, payload: string): string {
  if (root === 'applications://') return payload ? `${root}~${payload}` : root
  return root + payload
}
function virtualRootName(root: VirtualRoot): string {
  if (root === 'recent://') return t('最近使用')
  if (root === 'applications://') return t('应用程序')
  if (root === 'trash://') return t('废纸篓')
  return t('回收站')
}
/** Address-bar input accepts `recent`, `recent:/`, `recent://`, localized view
 *  names and Windows/Unix paths. Virtual schemes always normalize to one root
 *  and a single opaque payload segment; literal Chinese paths are encoded. */
export function normalizeAddress(input: string): string {
  const text = input.trim()
  if (!text) return text
  const label = text.replace(/[:：]+$/, '').replace(/[\\/]+$/, '')
  if (VIRTUAL_LABELS[label]) return VIRTUAL_LABELS[label]
  const bare = text.toLowerCase()
  if (['recent', 'applications', 'trash', 'recyclebin'].includes(bare)) return `${bare}://`
  // Both URL and Windows separators are accepted after a virtual scheme.
  const match = /^(recent|applications|trash|recyclebin):[\\/]*(.*)$/i.exec(text)
  if (!match) return text
  const root = `${match[1].toLowerCase()}://` as VirtualRoot
  let payload = match[2].replace(/^[\\/]+/, '')
  if (root === 'applications://') {
    if (/^user[\\/]/i.test(payload)) payload = payload.slice(5)
    else if (payload.startsWith('~')) payload = payload.slice(1)
    else if (payload && !opaquePayload(payload))
      payload = encodeBase64Url(payload.replace(/\\/g, '/'))
    return virtualHref(root, payload)
  }
  if (root === 'trash://') {
    if (payload && !opaquePayload(payload)) payload = encodeBase64Url(payload.replace(/\\/g, '/'))
    return virtualHref(root, payload)
  }
  return virtualHref(root, payload)
}
/** Human-readable address for the path editor and info dialogs. */
export function displayAddress(path: string): string {
  const virtual = virtualPath(path)
  if (!virtual) return path
  if (!virtual.payload) return virtual.root
  if (virtual.root === 'applications://' || virtual.root === 'trash://') {
    const relative = decodeBase64Url(virtual.payload)
    if (relative) return virtual.root + relative
  }
  return path
}
export function corePath(path: string): CorePath | null {
  if (!/^fiex:/i.test(path)) return null
  const match = path.match(/^fiex:\/\/mount\/([A-Za-z0-9-]{1,128})\/(.*)$/)
  if (!match || path.length > 32768 || match[2].split('/').length > 256)
    throw new Error('InvalidVfsUri')
  const parts = match[2]
    ? match[2].split('/').map((part) => {
        if (/[?#\\]/.test(part)) throw new Error('InvalidVfsUri')
        const decoded = decodeURIComponent(part)
        if (
          !decoded ||
          decoded.length > 1024 ||
          ['.', '..'].includes(decoded) ||
          /[/\\\0:\x00-\x1f\x7f]/.test(decoded) ||
          /%(?:2e|2f|5c|00|25)/i.test(decoded)
        )
          throw new Error('InvalidVfsUri')
        return decoded
      })
    : []
  return { root: `fiex://mount/${match[1]}/`, parts }
}
export function parent(path: string): string {
  const core = corePath(path)
  if (core) return core.root + core.parts.slice(0, -1).map(encodeURIComponent).join('/')
  const virtual = virtualPath(path)
  if (virtual) {
    if (!virtual.payload || virtual.root === 'recent://' || virtual.root === 'recyclebin://')
      return virtual.root
    const relative = decodeBase64Url(virtual.payload)
    if (!relative || !relative.includes('/')) return virtual.root
    return virtualHref(virtual.root, encodeBase64Url(relative.slice(0, relative.lastIndexOf('/'))))
  }
  const p = path.replace(/[\\/]+$/, '')
  const sep = p.includes('\\') ? '\\' : '/'
  const i = p.lastIndexOf(sep)
  if (i < 0) return path
  if (i === 0) return '/'
  const root = p.slice(0, i)
  return /^[A-Za-z]:$/.test(root) ? root + '\\' : root
}
export function join(path: string, name: string): string {
  const core = corePath(path)
  if (core) {
    const result = core.root + [...core.parts, name].map(encodeURIComponent).join('/')
    corePath(result)
    return result
  }
  const virtual = virtualPath(path)
  if (virtual && (virtual.root === 'applications://' || virtual.root === 'trash://')) {
    const relative = virtual.payload ? decodeBase64Url(virtual.payload) : ''
    if (relative !== null)
      return virtualHref(virtual.root, encodeBase64Url(relative ? `${relative}/${name}` : name))
  }
  return path.replace(/[\\/]+$/, '') + (path.includes('\\') ? '\\' : '/') + name
}
export type Crumb = { name: string; path: string }
export function pathCrumbs(
  path: string,
  displayName: (path: string) => string = (value) => basename(value),
): Crumb[] {
  const core = corePath(path)
  if (core)
    return [
      { name: displayName(core.root) || t('虚拟文件系统'), path: core.root },
      ...core.parts.map((name, index) => ({
        name,
        path:
          core.root +
          core.parts
            .slice(0, index + 1)
            .map(encodeURIComponent)
            .join('/'),
      })),
    ]
  const virtual = virtualPath(path)
  if (virtual) {
    const root = virtual.root
    const crumbs: Crumb[] = [{ name: displayName(root) || virtualRootName(root), path: root }]
    // Recycle-bin identities are opaque records, not navigable folders.
    if (virtual.payload && root !== 'recyclebin://') {
      const relative = opaquePayload(virtual.payload)
      if (relative) {
        let prefix = ''
        for (const part of relative.split('/').filter(Boolean)) {
          prefix = prefix ? `${prefix}/${part}` : part
          crumbs.push({
            name: part,
            path: virtualHref(root, encodeBase64Url(prefix)),
          })
        }
      } else {
        const child = virtualHref(root, virtual.payload)
        crumbs.push({ name: displayName(child) || basename(child), path: child })
      }
    }
    return crumbs
  }
  let accumulated = path.startsWith('/') ? '' : path.startsWith('\\') ? '\\\\' : ''
  return path
    .split(/[\\/]/)
    .filter(Boolean)
    .map((name) => {
      accumulated = accumulated
        ? join(accumulated, name)
        : path.startsWith('/')
          ? '/' + name
          : name + '\\'
      return { name: displayName(accumulated) || name, path: accumulated }
    })
}
export function basename(path: string): string {
  let core: CorePath | null = null
  try {
    core = corePath(path)
  } catch {
    return path
  }
  if (core) return core.parts.at(-1) || t('虚拟文件系统')
  const virtual = virtualPath(path)
  if (virtual) {
    if (!virtual.payload) return virtualRootName(virtual.root)
    if (virtual.root === 'applications://' || virtual.root === 'trash://') {
      const relative = decodeBase64Url(virtual.payload)
      if (relative) return relative.split('/').pop() || virtualRootName(virtual.root)
    }
    return virtual.payload
  }
  return (
    path
      .replace(/[\\/]+$/, '')
      .split(/[\\/]/)
      .pop() || path
  )
}
export function uniqueName(names: string[], preferred: string): string {
  const taken = new Set(names.map((name) => name.toLocaleLowerCase()))
  if (!taken.has(preferred.toLocaleLowerCase())) return preferred
  const dot = preferred.lastIndexOf('.')
  const stem = dot > 0 ? preferred.slice(0, dot) : preferred
  const extension = dot > 0 ? preferred.slice(dot) : ''
  for (let i = 2; i < 10_000; i++) {
    const candidate = `${stem} ${i}${extension}`
    if (!taken.has(candidate.toLocaleLowerCase())) return candidate
  }
  return `${stem} ${Date.now()}${extension}`
}

export function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 4)
  return `${(bytes / 1024 ** i).toFixed(i === 1 ? 0 : 1)} ${['B', 'KB', 'MB', 'GB', 'TB'][i]}`
}
export function date(timestamp: number): string {
  return timestamp
    ? new Intl.DateTimeFormat(getLocale(), { dateStyle: 'medium', timeStyle: 'short' }).format(
        new Date(timestamp * 1000),
      )
    : '—'
}
export function kind(e: Entry): string {
  if (
    e.extension === 'app' ||
    e.extension === 'exe' ||
    e.extension === 'desktop' ||
    e.extension === 'appimage'
  )
    return t('应用程序')
  if (e.isDir) return t('文件夹')
  if (['zip', '7z', 'rar', 'tar', 'gz', 'bz2', 'xz', 'tgz', 'zst', 'iso'].includes(e.extension))
    return t('归档')
  if (e.mime.startsWith('image/')) return t('图片')
  if (e.mime.startsWith('video/')) return t('视频')
  if (e.mime.startsWith('audio/')) return t('音频')
  if (['pdf', 'doc', 'docx', 'odt', 'pages', 'rtf'].includes(e.extension)) return t('文档')
  if (['xls', 'xlsx', 'csv', 'numbers'].includes(e.extension)) return t('表格')
  if (['ppt', 'pptx', 'key'].includes(e.extension)) return t('演示文稿')
  if (
    e.mime.startsWith('text/') ||
    [
      'json',
      'ts',
      'tsx',
      'js',
      'jsx',
      'rs',
      'py',
      'c',
      'h',
      'cpp',
      'go',
      'toml',
      'yaml',
      'yml',
      'xml',
      'sh',
      'swift',
      'vue',
      'svelte',
    ].includes(e.extension)
  )
    return t('文本与代码')
  if (['ttf', 'otf', 'woff', 'woff2'].includes(e.extension)) return t('字体')
  return e.extension ? t('{0} 文件', e.extension.toUpperCase()) : t('文件')
}
export function groupKey(e: Entry, by: Group, now = Date.now()): string {
  switch (by) {
    case 'kind':
      return kind(e)
    case 'extension':
      return e.isDir ? t('文件夹') : e.extension.toUpperCase() || t('无扩展名')
    case 'modified': {
      const today = new Date(now)
      today.setHours(0, 0, 0, 0)
      const diff = (today.getTime() - e.modified * 1000) / 86400000
      return diff <= 0
        ? t('今天')
        : diff <= 1
          ? t('昨天')
          : diff <= 7
            ? t('过去 7 天')
            : diff <= 30
              ? t('过去 30 天')
              : t('更早')
    }
    case 'size':
      return e.isDir
        ? t('文件夹')
        : e.size < 1024 ** 2
          ? t('小于 1 MB')
          : e.size < 100 * 1024 ** 2
            ? '1–100 MB'
            : e.size < 1024 ** 3
              ? '100 MB–1 GB'
              : t('大于 1 GB')
    default:
      return ''
  }
}
const collators = new Map<string, Intl.Collator>()
const collator = {
  compare: (a: string, b: string) => {
    const locale = getLocale()
    if (!collators.has(locale))
      collators.set(locale, new Intl.Collator(locale, { numeric: true, sensitivity: 'base' }))
    return collators.get(locale)!.compare(a, b)
  },
}
export function sortEntries(entries: Entry[], sort: Sort, desc: boolean): Entry[] {
  return [...entries].sort(
    (a, b) =>
      Number(b.isDir) - Number(a.isDir) ||
      (desc ? -1 : 1) *
        (sort === 'name'
          ? collator.compare(a.name, b.name)
          : sort === 'kind'
            ? collator.compare(kind(a), kind(b)) || collator.compare(a.name, b.name)
            : a[sort] - b[sort] || collator.compare(a.name, b.name)),
  )
}
/** Sort for a clicked list header: a different column starts ascending, the
 *  active column flips direction. Folder-first ordering stays in sortEntries. */
export function nextSort(
  sort: Sort,
  descending: boolean,
  key: Sort,
): { sort: Sort; descending: boolean } {
  return { sort: key, descending: sort === key ? !descending : false }
}
export function groups(entries: Entry[], by: Group): [string, Entry[]][] {
  const map = new Map<string, Entry[]>()
  for (const e of entries) {
    const key = groupKey(e, by)
    if (!map.has(key)) map.set(key, [])
    map.get(key)!.push(e)
  }
  return [...map]
}
export function isImage(e: Entry) {
  return (
    e.mime.startsWith('image/') &&
    !['icns', 'ico', 'heic', 'heif', 'tif', 'tiff', 'psd', 'raw'].includes(e.extension)
  )
}
