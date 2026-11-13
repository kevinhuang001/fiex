export type BatchRename = { path: string; name: string }
export type RenameEntry = (path: string, name: string) => Promise<string>
export type TemporaryName = (index: number) => string

/** Stage every source before assigning final names so swaps remain possible. */
export async function batchRenameEntries(
  renames: BatchRename[],
  renameEntry: RenameEntry,
  temporaryName: TemporaryName = (index) =>
    `.fiex-batch-${crypto.randomUUID().slice(0, 8)}-${index}`,
): Promise<string[]> {
  const staged: { original: string; temp: string; name: string }[] = []
  const finalized: { path: string; original: string; temp: string }[] = []
  try {
    for (const [index, item] of renames.entries()) {
      const temp = await renameEntry(item.path, temporaryName(index))
      staged.push({ original: item.path, temp, name: item.name })
    }
    for (const item of staged) {
      const path = await renameEntry(item.temp, item.name)
      finalized.push({ path, original: item.original, temp: item.temp })
    }
    return finalized.map((item) => item.path)
  } catch (error) {
    // Move successful final names back to temporary names before restoring the
    // originals, so swaps can roll back without colliding with each other.
    for (const item of finalized.reverse()) {
      await renameEntry(item.path, basename(item.temp)).catch(() => {})
    }
    for (const item of staged.reverse()) {
      await renameEntry(item.temp, basename(item.original)).catch(() => {})
    }
    throw error
  }
}

function basename(path: string): string {
  return (
    path
      .replace(/[\\/]+$/, '')
      .split(/[\\/]/)
      .pop() ?? path
  )
}
