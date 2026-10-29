/** A preview provider returned by Core. Matching, priority and built-in
 * capability checks live in Core; the browser only applies user preferences. */
import { call } from './api'
export interface PreviewCandidate {
  id: string
  provider: string
  priority: number
  /** Set by Core when the plugin declares `extensions.preview.loading`. */
  loading?: boolean
}

const PLUGIN_PREFIX = 'plugin:'

/** Plugin id behind a `plugin:<id>` preview provider, or null for built-ins. */
export function pluginPreviewId(providerId?: string | null): string | null {
  return providerId?.startsWith(PLUGIN_PREFIX) ? providerId.slice(PLUGIN_PREFIX.length) : null
}

const ORDER_KEY = 'fiex.preview.order'
const DISABLED_KEY = 'fiex.preview.disabled'

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

const migrated = () => Boolean(localStorage.getItem('fiex.provider.preview.migrated'))
const previewOrder = () => (migrated() ? [] : readList(ORDER_KEY))
export const previewDisabled = () => (migrated() ? [] : readList(DISABLED_KEY))

/** Preserve existing preview choices when upgrading to the shared Core store. */
export async function migratePreviewPreferences(): Promise<void> {
  if (localStorage.getItem('fiex.provider.preview.migrated')) return
  const order = readList(ORDER_KEY)
  const disabled = readList(DISABLED_KEY)
  const current =
    await call<Record<string, { order: string[]; disabled: string[] }>>('provider_preferences')
  if (!current.preview && (order.length || disabled.length)) {
    await call('provider_preferences_set', { feature: 'preview', preference: { order, disabled } })
  }
  localStorage.setItem('fiex.provider.preview.migrated', '1')
}

export function orderCandidates(candidates: PreviewCandidate[]): PreviewCandidate[] {
  if (migrated()) return candidates
  const order = previewOrder()
  const rank = (id: string) => {
    const index = order.indexOf(id)
    return index === -1 ? Number.POSITIVE_INFINITY : index
  }
  return [...candidates].sort(
    (a, b) => rank(a.id) - rank(b.id) || b.priority - a.priority || a.id.localeCompare(b.id),
  )
}
