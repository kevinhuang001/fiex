import { parentDirectoryOf } from '@/lib/path'
import type { Entry } from '@/lib/types'

/** Whether the destination supports duplicating every selected entry. */
export function canDuplicateEntries(
  entries: Array<Entry | undefined>,
  allowedOps: string[],
): boolean {
  if (!entries.length || entries.some((entry) => !entry)) return false
  const needsDirectory = entries.some((entry) => entry?.isDir)
  const needsFile = entries.some((entry) => !entry?.isDir)
  return (
    (!needsDirectory || allowedOps.includes('mkdir')) &&
    (!needsFile || allowedOps.includes('write'))
  )
}

/** Remove moves that are already satisfied and collect directories to refresh. */
export function planTransferPaths(
  paths: string[],
  destination: string,
  moving: boolean,
  sourceDirectories: Array<string | undefined>,
) {
  const sourceParents = moving
    ? paths.map((path, index) => sourceDirectories[index] ?? parentDirectoryOf(path))
    : []
  const actionablePaths = moving
    ? paths.filter((path, index) => path !== destination && sourceParents[index] !== destination)
    : paths
  return {
    actionablePaths,
    affectedSourceDirectories: sourceParents,
  }
}
