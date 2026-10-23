import type { Entry, Group, Sort } from './types'
export function parent(path: string): string {
  if (/^history:\/\/[^/]+\/[^/]+\/?$/.test(path)) return path.replace(/\/$/, '')
  const p = path.replace(/[\\/]+$/, '')
  const sep = p.includes('\\') ? '\\' : '/'
  const i = p.lastIndexOf(sep)
  if (i < 0) return path
  if (i === 0) return '/'
  const root = p.slice(0, i)
  return /^[A-Za-z]:$/.test(root) ? root + '\\' : root
}
export function join(path: string, name: string): string {
  return path.replace(/[\\/]+$/, '') + (path.includes('\\') ? '\\' : '/') + name
}
export function basename(path: string): string {
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
    ? new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }).format(
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
    return '应用程序'
  if (e.isDir) return '文件夹'
  if (['zip', '7z', 'rar', 'tar', 'gz', 'bz2', 'xz', 'tgz', 'zst', 'iso'].includes(e.extension))
    return '归档'
  if (e.mime.startsWith('image/')) return '图片'
  if (e.mime.startsWith('video/')) return '视频'
  if (e.mime.startsWith('audio/')) return '音频'
  if (['pdf', 'doc', 'docx', 'odt', 'pages', 'rtf'].includes(e.extension)) return '文档'
  if (['xls', 'xlsx', 'csv', 'numbers'].includes(e.extension)) return '表格'
  if (['ppt', 'pptx', 'key'].includes(e.extension)) return '演示文稿'
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
    return '文本与代码'
  if (['ttf', 'otf', 'woff', 'woff2'].includes(e.extension)) return '字体'
  return e.extension ? `${e.extension.toUpperCase()} 文件` : '文件'
}
export function groupKey(e: Entry, by: Group, now = Date.now()): string {
  switch (by) {
    case 'kind':
      return kind(e)
    case 'extension':
      return e.isDir ? '文件夹' : e.extension.toUpperCase() || '无扩展名'
    case 'modified': {
      const today = new Date(now)
      today.setHours(0, 0, 0, 0)
      const diff = (today.getTime() - e.modified * 1000) / 86400000
      return diff <= 0
        ? '今天'
        : diff <= 1
          ? '昨天'
          : diff <= 7
            ? '过去 7 天'
            : diff <= 30
              ? '过去 30 天'
              : '更早'
    }
    case 'size':
      return e.isDir
        ? '文件夹'
        : e.size < 1024 ** 2
          ? '小于 1 MB'
          : e.size < 100 * 1024 ** 2
            ? '1–100 MB'
            : e.size < 1024 ** 3
              ? '100 MB–1 GB'
              : '大于 1 GB'
    default:
      return ''
  }
}
const collator = new Intl.Collator('zh-CN', { numeric: true, sensitivity: 'base' })
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
export function isText(e: Entry) {
  return kind(e) === '文本与代码' || ['md', 'log', 'txt', 'csv', 'svg'].includes(e.extension)
}
