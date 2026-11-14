import { t, getLocale } from '@/i18n'
import type { Entry, Group } from '../types'

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
