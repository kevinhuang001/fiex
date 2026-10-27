import { t } from '@/i18n'
import type { DecorationProvider } from '../types'

export const readonlyDecorationProvider: DecorationProvider = {
  id: 'builtin:readonly',
  priority: 0,
  resolve: (entry) =>
    entry.readonly
      ? {
          path: entry.path,
          badges: [{ text: t('只读'), color: 'gray', tooltip: t('只读') }],
          providers: ['Fiex'],
        }
      : undefined,
}
