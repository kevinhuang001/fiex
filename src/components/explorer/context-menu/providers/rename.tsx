import { t } from '@/i18n'
import { Pencil } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const renameProvider: ContextMenuProvider = {
  id: 'builtin:rename',
  order: 40,
  matches: ({ paths }) => paths.length === 1,
  resolve: ({ menu, hint, x }) => [
    {
      label: t('重新命名'),
      icon: <Pencil />,
      hint: hint('rename'),
      disabled: x.directoryReadOnly,
      action: () => x.beginRename(menu.entry.path),
    },
  ],
}
