import type { PreviewCandidate } from '@/lib/preview'

const PLUGIN_PREFIX = 'plugin:'

/** Plugin id behind a `plugin:<id>` preview provider, or null for built-ins. */
export function pluginPreviewId(providerId?: string | null): string | null {
  return providerId?.startsWith(PLUGIN_PREFIX) ? providerId.slice(PLUGIN_PREFIX.length) : null
}

/** Apply the saved order, then stable provider priority and id tie-breakers. */
export function orderCandidates(
  candidates: PreviewCandidate[],
  preferredOrder: string[],
): PreviewCandidate[] {
  const rank = (id: string) => {
    const index = preferredOrder.indexOf(id)
    return index === -1 ? Number.POSITIVE_INFINITY : index
  }
  return [...candidates].sort(
    (a, b) => rank(a.id) - rank(b.id) || b.priority - a.priority || a.id.localeCompare(b.id),
  )
}
