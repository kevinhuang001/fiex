import { t } from '@/i18n'
import { Tags } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const tagsProvider: ContextMenuProvider = {
  id: 'builtin:tags',
  order: 20,
  matches: ({ platform }) => platform === 'macos',
  resolve: ({ openTags, x, paths }) => [
    {
      label: t('标签…'),
      disabled: x.directoryReadOnly,
      icon: <Tags />,
      hint: '',
      action: () => openTags(paths),
    },
  ],
}
