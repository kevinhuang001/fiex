import { t } from '@/i18n'
import { Copy, Scissors } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const clipboardProvider: ContextMenuProvider = {
  id: 'builtin:clipboard',
  order: 50,
  matches: () => true,
  resolve: ({ hint, copy, duplicate, x }) => [
    { label: t('复制'), icon: <Copy />, hint: hint('copy'), action: () => copy(false) },
    {
      label: t('剪切'),
      icon: <Scissors />,
      hint: hint('cut'),
      disabled: x.directoryReadOnly,
      action: () => copy(true),
    },
    {
      label: t('创建副本'),
      icon: <Copy />,
      hint: hint('duplicate'),
      disabled: x.directoryReadOnly,
      action: duplicate,
    },
  ],
}
