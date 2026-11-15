import { useMemo } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import type { Entry } from '@/lib/types'
import { canRenameEntry, renameSingleEntry } from '../rename'

export interface InlineRenameServices {
  renameEntry: (path: string, name: string) => Promise<string>
  renameUnsupportedMessage: () => string
}

export interface InlineRenameOptions {
  path: string
  entries: Entry[]
  setEntries: Dispatch<SetStateAction<Entry[]>>
  setSelected: (paths: string[]) => void
  setRenaming: (target: string | null) => void
  setError: (message: string) => void
  refreshDirectory: (paths: string[]) => void
  services: InlineRenameServices
}

export function createInlineRenameActions({
  path,
  entries,
  setEntries,
  setSelected,
  setRenaming,
  setError,
  refreshDirectory,
  services,
}: InlineRenameOptions) {
  const beginRename = (target: string, createdEntry?: Entry) => {
    const entry = entries.find((item) => item.path === target) ?? createdEntry
    if (entry?.path !== target || !canRenameEntry(entry)) return
    if (!entries.some((item) => item.path === target))
      setEntries((previous) => [...previous, entry])
    setSelected([target])
    setRenaming(target)
  }

  const cancelRename = () => setRenaming(null)

  const renameTo = async (target: string, name: string) => {
    const result = await renameSingleEntry(target, name, entries, (resourcePath, nextName) =>
      services.renameEntry(resourcePath, nextName),
    )
    if (result.status === 'unsupported') {
      setError(services.renameUnsupportedMessage())
      return false
    }
    if (result.status === 'failed') {
      setError(result.message)
      return false
    }
    setError('')
    setRenaming(null)
    setSelected([result.path])
    refreshDirectory([path])
    return true
  }

  return { beginRename, cancelRename, renameTo }
}

/** Binds rename actions to one pane's current listing and navigation state. */
export function useInlineRename(options: InlineRenameOptions) {
  const {
    path,
    entries,
    setEntries,
    setSelected,
    setRenaming,
    setError,
    refreshDirectory,
    services,
  } = options
  return useMemo(
    () =>
      createInlineRenameActions({
        path,
        entries,
        setEntries,
        setSelected,
        setRenaming,
        setError,
        refreshDirectory,
        services,
      }),
    [entries, path, refreshDirectory, services, setEntries, setError, setRenaming, setSelected],
  )
}
