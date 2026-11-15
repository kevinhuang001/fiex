import type { Dispatch, SetStateAction } from 'react'
import { t } from '@/i18n'
import type { Entry } from '@/lib/types'
import { batchRenameEntries } from '../batchRename'
import type { ExplorerOperationState, FileOperationServices } from '../types'

/** Batch rename dialog actions and post-commit pane reconciliation. */
export function createBatchRenameActions({
  x,
  services,
  notify,
  setBatchRename,
}: {
  x: Pick<ExplorerOperationState, 'selected' | 'entries' | 'setSelected' | 'reload'>
  services: Pick<FileOperationServices, 'renameEntry'>
  notify: (message: string) => void
  setBatchRename: Dispatch<SetStateAction<Entry[] | null>>
}) {
  const openBatchRename = (paths = x.selected) => {
    const targets = x.entries.filter((entry) => paths.includes(entry.path))
    if (targets.length > 1) setBatchRename(targets)
  }
  const applyBatchRename = async (renames: { path: string; name: string }[]) => {
    const renamed = await batchRenameEntries(renames, services.renameEntry)
    setBatchRename(null)
    x.setSelected([])
    x.reload()
    notify(t('已重命名 {0} 个项目', renamed.length))
  }
  return { openBatchRename, applyBatchRename }
}
