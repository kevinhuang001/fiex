import { call } from '@/lib/api'
import type { Entry } from '@/lib/types'

export type SingleRenameResult =
  | { status: 'renamed'; path: string }
  | { status: 'unsupported' }
  | { status: 'failed'; message: string }

export type RenameInvoker = (path: string, name: string) => Promise<string>

export function canRenameEntry(entry: Entry | undefined): boolean {
  return Boolean(entry?.allowedOps?.includes('rename'))
}

/** Apply one rename after checking the capability reported for the entry. */
export async function renameSingleEntry(
  target: string,
  name: string,
  entries: Entry[],
  invoke: RenameInvoker = (path, nextName) => call('rename_entry', { path, name: nextName }),
): Promise<SingleRenameResult> {
  if (!canRenameEntry(entries.find((entry) => entry.path === target)))
    return { status: 'unsupported' }

  try {
    return { status: 'renamed', path: await invoke(target, name) }
  } catch (error) {
    return { status: 'failed', message: String(error) }
  }
}
