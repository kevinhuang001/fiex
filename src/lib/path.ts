export function parentDirectoryOf(path: string): string {
  const scheme = path.indexOf('://')
  if (scheme >= 0) {
    const rootEnd = scheme + 3
    const trimmed = path.replace(/\/+$/, '')
    const separator = trimmed.lastIndexOf('/')
    return separator < rootEnd ? path.slice(0, rootEnd) : trimmed.slice(0, separator)
  }
  const trimmed = path.replace(/[\\/]+$/, '')
  const separator = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'))
  if (separator < 0) return ''
  if (separator === 0) return trimmed.slice(0, 1)
  if (separator === 2 && /^[A-Za-z]:/.test(trimmed)) return trimmed.slice(0, 3)
  return trimmed.slice(0, separator)
}
