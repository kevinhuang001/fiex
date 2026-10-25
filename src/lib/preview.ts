import type { PluginInfo } from './plugins'

/** One thing that can render a preview for an extension. Built-ins participate
 * in the same ordering as plugin previewers, so a plugin can override PDF. */
export interface Previewer {
  /** `builtin:<kind>` or `<pluginId>:<previewId>`. */
  id: string
  title: string
  extensions: string[]
  priority: number
  builtin: 'text' | 'image' | 'media' | 'pdf' | 'fallback' | null
  pluginId?: string
  panelId?: string
}

const ORDER_KEY = 'fiex.preview.order'
const DISABLED_KEY = 'fiex.preview.disabled'

export const BUILTIN_PREVIEWERS: Previewer[] = [
  { id: 'builtin:text', title: '文本', extensions: [], priority: 10, builtin: 'text' },
  { id: 'builtin:image', title: '图片', extensions: [], priority: 20, builtin: 'image' },
  { id: 'builtin:media', title: '音视频', extensions: [], priority: 30, builtin: 'media' },
  { id: 'builtin:pdf', title: 'PDF', extensions: ['pdf'], priority: 40, builtin: 'pdf' },
  { id: 'builtin:fallback', title: '文件信息', extensions: [], priority: 0, builtin: 'fallback' },
]

function readList(key: string): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(key) || '[]')
    return Array.isArray(value)
      ? value.filter((item): item is string => typeof item === 'string')
      : []
  } catch {
    return []
  }
}

function writeList(key: string, values: string[]) {
  try {
    localStorage.setItem(key, JSON.stringify(values))
  } catch {
    // Storage is best effort; preview falls back to declared priorities.
  }
}

export const previewOrder = () => readList(ORDER_KEY)
export const previewDisabled = () => readList(DISABLED_KEY)

export function movePreviewer(id: string, delta: number, all: Previewer[]) {
  const order = previewOrder().filter((item) => all.some((previewer) => previewer.id === item))
  const rest = all.map((previewer) => previewer.id).filter((item) => !order.includes(item))
  const list = [...order, ...rest]
  const index = list.indexOf(id)
  const target = Math.min(Math.max(index + delta, 0), list.length - 1)
  if (index < 0 || index === target) return
  list.splice(target, 0, ...list.splice(index, 1))
  writeList(ORDER_KEY, list)
}

export function togglePreviewer(id: string, enabled: boolean) {
  const disabled = new Set(previewDisabled())
  if (enabled) disabled.delete(id)
  else disabled.add(id)
  writeList(DISABLED_KEY, [...disabled])
}

/** Plugin previewers from the catalog. Preview panels are resolved by Core
 * through the same panel lookup as toolbar panels. */
export function pluginPreviewers(plugins: PluginInfo[]): Previewer[] {
  const values: Previewer[] = []
  for (const plugin of plugins) {
    const entries = plugin.extensions?.preview
    if (!Array.isArray(entries)) continue
    for (const entry of entries) {
      const id = typeof entry?.id === 'string' ? entry.id : ''
      const extensions = Array.isArray(entry?.extensions)
        ? entry.extensions
            .filter((item: unknown): item is string => typeof item === 'string')
            .map((item: string) => item.toLowerCase().replace(/^\./, ''))
        : []
      if (!id || !extensions.length) continue
      values.push({
        id: `${plugin.id}:${id}`,
        title: typeof entry.title === 'string' && entry.title ? entry.title : plugin.name,
        extensions,
        priority: typeof entry.priority === 'number' ? entry.priority : 100,
        builtin: null,
        pluginId: plugin.id,
        panelId: id,
      })
    }
  }
  return values
}

export function allPreviewers(plugins: PluginInfo[]): Previewer[] {
  return [...BUILTIN_PREVIEWERS, ...pluginPreviewers(plugins)]
}

/** Candidates for one extension, highest priority first, user order first. */
export function resolvePreviewer(plugins: PluginInfo[], extension: string): Previewer {
  const ext = extension.toLowerCase()
  const disabled = new Set(previewDisabled())
  const order = previewOrder()
  const rank = (id: string) => {
    const index = order.indexOf(id)
    return index === -1 ? Number.POSITIVE_INFINITY : index
  }
  const candidates = pluginPreviewers(plugins).filter(
    (previewer) => !disabled.has(previewer.id) && previewer.extensions.includes(ext),
  )
  candidates.sort(
    (a, b) => rank(a.id) - rank(b.id) || b.priority - a.priority || a.id.localeCompare(b.id),
  )
  if (candidates.length) return candidates[0]
  const builtin = BUILTIN_PREVIEWERS.find(
    (previewer) => !disabled.has(previewer.id) && previewer.extensions.includes(ext),
  )
  return builtin || BUILTIN_PREVIEWERS[BUILTIN_PREVIEWERS.length - 1]
}
