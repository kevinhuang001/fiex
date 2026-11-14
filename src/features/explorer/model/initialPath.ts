import type { Location, Preferences } from '@/lib/types'

/** Resolves the first pane path from explicit, persisted, and OS locations. */
export function resolveInitialPath({
  startOverride,
  launched,
  startPage,
  lastPath,
  initialPath,
  locations,
}: {
  startOverride?: string
  launched?: string | null
  startPage: Preferences['startPage']
  lastPath?: string | null
  initialPath?: string | null
  locations: Location[]
}): string | null {
  const explicit = startOverride || launched
  if (explicit) return explicit
  if (startPage === 'recent') return 'recent://'
  if (startPage === 'last' && lastPath) return lastPath

  const pick = (name: string) => locations.find((item) => item.name === name)?.path
  return startPage === 'home'
    ? (pick('个人文件夹') ?? initialPath ?? null)
    : (pick('文稿') ?? initialPath ?? locations[0]?.path ?? null)
}

/** Resolves startup sources in order and stops using results after cancellation. */
export async function resolveInitialPathRequest({
  startOverride,
  startPage,
  native,
  isCancelled,
  loadLaunchPath,
  loadLastPath,
  loadInitialPath,
  loadLocations,
}: {
  startOverride?: string
  startPage: Preferences['startPage']
  native: boolean
  isCancelled: () => boolean
  loadLaunchPath: () => Promise<string | null>
  loadLastPath: () => Promise<string | null>
  loadInitialPath: () => Promise<string | null>
  loadLocations: () => Promise<Location[]>
}): Promise<string | null> {
  const launched = native ? await loadLaunchPath().catch(() => null) : null
  if (isCancelled()) return null

  let lastPath: string | null = null
  if (!startOverride && !launched && startPage === 'last') {
    lastPath = await loadLastPath().catch(() => null)
  }
  if (isCancelled()) return null

  const needsLocation = !startOverride && !launched && startPage !== 'recent' && !lastPath
  const initialPath = needsLocation && native ? await loadInitialPath().catch(() => null) : null
  if (isCancelled()) return null

  const locations = needsLocation ? await loadLocations().catch(() => []) : []
  if (isCancelled()) return null

  return resolveInitialPath({
    ...(startOverride === undefined ? {} : { startOverride }),
    launched,
    startPage,
    lastPath,
    initialPath,
    locations,
  })
}
