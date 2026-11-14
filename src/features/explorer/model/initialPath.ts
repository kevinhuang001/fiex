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
