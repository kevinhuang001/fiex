export type ListingRefreshTarget = 'search' | 'directory'

export function listingRefreshTarget(query: string): ListingRefreshTarget {
  return query.trim() ? 'search' : 'directory'
}

export function connectionContainsPath(path: string, mountPath: string): boolean {
  const normalizedMount = mountPath.replace(/\/$/, '')
  return path === mountPath || path.startsWith(`${normalizedMount}/`)
}

export function directoryChangeIncludesPath(detail: unknown, path: string): boolean {
  if (!detail || typeof detail !== 'object') return false
  const directories = (detail as { directories?: unknown }).directories
  return Array.isArray(directories) && directories.includes(path)
}

export function shouldWatchDirectory(
  state: { enabled: boolean; native: boolean; path: string; virtualVolume: boolean },
  isVirtualResource: (path: string) => boolean,
): boolean {
  return (
    state.enabled &&
    state.native &&
    Boolean(state.path) &&
    !state.virtualVolume &&
    !isVirtualResource(state.path) &&
    !state.path.startsWith('fiex-plugin-view://')
  )
}

export function connectionCoversPath(path: string, connections: { mountPath: string }[]): boolean {
  return connections.some((connection) => connectionContainsPath(path, connection.mountPath))
}
