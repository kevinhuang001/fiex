import { t } from '@/i18n'
import { Pencil } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const batchRenameProvider: ContextMenuProvider = {
  id: 'builtin:batchRename',
  order: 45,
  matches: ({ paths }) => paths.length > 1,
  resolve: ({ x, openBatchRename }) => [
    {
      label: t('批量重命名…'),
      icon: <Pencil />,
      hint: '',
      disabled: x.directoryReadOnly,
      action: openBatchRename,
    },
  ],
}
