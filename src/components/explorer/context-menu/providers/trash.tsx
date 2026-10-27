import { t } from '@/i18n'
import { RotateCcw, Trash2 } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const trashProvider: ContextMenuProvider = {
  id: 'builtin:trash',
  order: 100,
  placement: 'tail',
  matches: () => true,
  resolve: ({ inTrash, restoreFromTrash, recycle, deletePermanently, hint, x, paths }) => [
    inTrash
      ? {
          label: t('放回原处'),
          icon: <RotateCcw />,
          hint: '',
          action: () => restoreFromTrash(paths),
        }
      : {
          label: t('移到废纸篓'),
          icon: <Trash2 />,
          hint: hint('trash'),
          disabled: x.directoryReadOnly,
          danger: true,
          action: recycle,
        },
    ...(inTrash
      ? [
          {
            label: t('彻底删除'),
            icon: <Trash2 />,
            hint: '',
            danger: true,
            action: () => deletePermanently(paths),
          },
        ]
      : []),
  ],
}
