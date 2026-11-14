import type { Location } from '@/lib/types'

export interface LocationRefreshCoordinator {
  refresh(force?: boolean): Promise<Location[]>
}

/** Coalesces location queries and ignores stale responses after a forced refresh. */
export function createLocationRefreshCoordinator(
  load: () => Promise<Location[]>,
  onChange: (locations: Location[]) => void,
): LocationRefreshCoordinator {
  let request = 0
  let currentKey = ''
  let pending: Promise<Location[]> | null = null

  return {
    refresh(force = false) {
      if (pending && !force) return pending
      const generation = ++request
      const next = load()
        .then((locations) => {
          if (generation !== request) return locations
          const key = JSON.stringify(locations)
          if (key !== currentKey) {
            currentKey = key
            onChange(locations)
          }
          return locations
        })
        .finally(() => {
          if (generation === request) pending = null
        })
      pending = next
      return next
    },
  }
}
