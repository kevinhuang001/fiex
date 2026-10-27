import { t } from '@/i18n'
import { ExternalLink, Eye, Info } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const openingProvider: ContextMenuProvider = {
  id: 'builtin:opening',
  order: 10,
  matches: () => true,
  resolve: ({ menu, hint, open, previewEntry, setInfo, multiple }) => [
    {
      label: t('打开'),
      icon: <ExternalLink />,
      hint: hint('open'),
      action: () => open(menu.entry),
    },
    ...(multiple
      ? []
      : [
          {
            label: t('快速查看'),
            icon: <Eye />,
            hint: hint('quickLook'),
            action: () => previewEntry(menu.entry),
          },
          {
            label: t('显示简介'),
            icon: <Info />,
            hint: hint('info'),
            action: () => setInfo(menu.entry),
          },
        ]),
  ],
}
