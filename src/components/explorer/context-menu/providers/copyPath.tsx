import { t } from '@/i18n'
import { Copy } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const copyPathProvider: ContextMenuProvider = {
  id: 'builtin:copy-path',
  order: 70,
  matches: () => true,
  resolve: ({ menu, notify, x }) => [
    {
      label: t('复制路径'),
      icon: <Copy />,
      hint: '',
      action: () =>
        void navigator.clipboard
          .writeText(menu.entry.path)
          .then(() => notify(t('路径已复制')))
          .catch((e) => x.setError(String(e))),
    },
  ],
}
