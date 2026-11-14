import { t } from '@/i18n'

export type CorePath = { root: string; parts: string[] }
export type VirtualRoot = 'recent://' | 'applications://' | 'trash://' | 'recyclebin://'
export type VirtualAddress = { root: VirtualRoot; payload: string }
/** VFS identities are not host filesystem paths usable by terminals or IDEs. */
export function isVirtualResource(path: string): boolean {
  return /^[a-z][a-z\d+.-]*:\/\//i.test(path)
}
const VIRTUAL_LABELS: Record<string, VirtualRoot> = {
  最近使用: 'recent://',
  应用程序: 'applications://',
  应用: 'applications://',
  废纸篓: 'trash://',
  回收站: 'recyclebin://',
}
/** Recognize the Core-owned virtual views without treating Windows drives or
 *  `fiex://mount/...` identities as virtual application paths. */
export function virtualPath(path: string): VirtualAddress | null {
  const match = /^(recent|applications|trash|recyclebin):\/\/(.*)$/i.exec(path)
  if (!match || match[1] === undefined || match[2] === undefined) return null
  const root = `${match[1].toLowerCase()}://` as VirtualRoot
  return { root, payload: match[2] }
}
function virtualHref(root: VirtualRoot, payload: string): string {
  return root + payload
}
function encodeVirtualSegment(segment: string): string {
  let decoded = segment
  try {
    decoded = decodeURIComponent(segment)
  } catch {
    // Treat a malformed percent sequence as a literal name entered by the user.
  }
  return encodeURIComponent(decoded)
}
function decodeVirtualSegment(segment: string): string {
  try {
    return decodeURIComponent(segment)
  } catch {
    return segment
  }
}
function virtualRootName(root: VirtualRoot): string {
  if (root === 'recent://') return t('最近使用')
  if (root === 'applications://') return t('应用程序')
  if (root === 'trash://') return t('废纸篓')
  return t('回收站')
}
/** Address-bar input accepts view roots, localized names and Windows/Unix
 *  paths. Core VFS paths keep the same hierarchy as their browser views. */
export function normalizeAddress(input: string): string {
  const text = input.trim()
  if (!text) return text
  const label = text.replace(/[:：]+$/, '').replace(/[\\/]+$/, '')
  if (VIRTUAL_LABELS[label]) return VIRTUAL_LABELS[label]
  const bare = text.toLowerCase()
  if (['recent', 'applications', 'trash', 'recyclebin'].includes(bare)) return `${bare}://`
  // Both URL and Windows separators are accepted after a virtual scheme.
  const match = /^(recent|applications|trash|recyclebin):[\\/]*(.*)$/i.exec(text)
  if (!match || match[1] === undefined || match[2] === undefined) return text
  const root = `${match[1].toLowerCase()}://` as VirtualRoot
  const payload = match[2].replace(/^[\\/]+/, '')
  if (root === 'applications://' || root === 'trash://')
    return virtualHref(
      root,
      payload.replace(/\\/g, '/').split('/').filter(Boolean).map(encodeVirtualSegment).join('/'),
    )
  if ((root === 'recent://' || root === 'recyclebin://') && payload) {
    const queryIndex = payload.indexOf('?')
    const name = queryIndex < 0 ? payload : payload.slice(0, queryIndex)
    const query = queryIndex < 0 ? '' : payload.slice(queryIndex)
    return virtualHref(root, `${encodeVirtualSegment(name)}${query}`)
  }
  return virtualHref(root, payload)
}
/** Human-readable address for the path editor and info dialogs. */
export function displayAddress(path: string): string {
  const virtual = virtualPath(path)
  if (!virtual) return path
  if (!virtual.payload) return virtual.root
  if (virtual.root === 'trash://' || virtual.root === 'applications://') {
    return (
      virtual.root + virtual.payload.split('/').filter(Boolean).map(decodeVirtualSegment).join('/')
    )
  }
  if (virtual.root === 'recent://' || virtual.root === 'recyclebin://') {
    const queryIndex = virtual.payload.indexOf('?')
    const name = queryIndex < 0 ? virtual.payload : virtual.payload.slice(0, queryIndex)
    return virtual.root + decodeVirtualSegment(name)
  }
  return path
}
export function corePath(path: string): CorePath | null {
  if (!/^fiex:/i.test(path)) return null
  const match = path.match(/^fiex:\/\/mount\/([A-Za-z0-9-]{1,128})\/(.*)$/)
  if (
    !match ||
    match[1] === undefined ||
    match[2] === undefined ||
    path.length > 32768 ||
    match[2].split('/').length > 256
  )
    throw new Error('InvalidVfsUri')
  const parts = match[2]
    ? match[2].split('/').map((part) => {
        if (/[?#\\]/.test(part)) throw new Error('InvalidVfsUri')
        const decoded = decodeURIComponent(part)
        if (
          !decoded ||
          decoded.length > 1024 ||
          ['.', '..'].includes(decoded) ||
          /[/\\\0:\x00-\x1f\x7f]/.test(decoded) || // eslint-disable-line no-control-regex -- rejects control characters in decoded VFS segments
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
    if (!virtual.payload || virtual.root === 'recyclebin://' || virtual.root === 'recent://')
      return virtual.root
    if (virtual.root === 'applications://' || virtual.root === 'trash://') {
      const segments = virtual.payload.split('/').filter(Boolean)
      segments.pop()
      return segments.length ? virtualHref(virtual.root, segments.join('/')) : virtual.root
    }
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
    return virtualHref(
      virtual.root,
      virtual.payload
        ? `${virtual.payload.replace(/\\/g, '/')}/${encodeVirtualSegment(name)}`
        : encodeVirtualSegment(name),
    )
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
    // Recent and Windows Recycle Bin are flat views. Trash and Applications
    // expose their browsed folder hierarchy directly in URI path segments.
    if (virtual.payload) {
      if (root === 'applications://' || root === 'trash://') {
        let prefix = ''
        for (const part of virtual.payload.split('/').filter(Boolean)) {
          prefix = prefix ? `${prefix}/${part}` : part
          const node = virtualHref(root, prefix)
          crumbs.push({ name: displayName(node) || basename(node), path: node })
        }
        return crumbs
      }
      if (root === 'recent://' || root === 'recyclebin://') {
        const segment = virtual.payload.split('?')[0] ?? ''
        const child = virtualHref(root, virtual.payload)
        crumbs.push({ name: displayName(child) || decodeVirtualSegment(segment), path: child })
        return crumbs
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
  try {
    const core = corePath(path)
    if (core) return core.parts.at(-1) || t('虚拟文件系统')
  } catch {
    return path
  }
  const virtual = virtualPath(path)
  if (virtual) {
    if (!virtual.payload) return virtualRootName(virtual.root)
    if (virtual.root === 'trash://' || virtual.root === 'applications://') {
      const segment = virtual.payload.split('/').at(-1) || ''
      return decodeVirtualSegment(segment) || virtualRootName(virtual.root)
    }
    if (virtual.root === 'recent://' || virtual.root === 'recyclebin://') {
      return (
        decodeVirtualSegment(virtual.payload.split('?')[0] ?? '') || virtualRootName(virtual.root)
      )
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
  for (let i = 1; i < 10_000; i++) {
    const candidate = `${stem} (${i})${extension}`
    if (!taken.has(candidate.toLocaleLowerCase())) return candidate
  }
  return `${stem} (${Date.now()})${extension}`
}
