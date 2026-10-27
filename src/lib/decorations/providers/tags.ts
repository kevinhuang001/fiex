import { tagHex } from '@/lib/tags'
import type { DecorationProvider } from '../types'

export const tagDecorationProvider: DecorationProvider = {
  id: 'builtin:tags',
  priority: -10,
  resolve: (entry) =>
    entry.tags.length
      ? {
          path: entry.path,
          badges: entry.tags.slice(0, 5).map((tag) => ({
            text: tag.name,
            tooltip: tag.name,
            color: tagHex(tag),
            dot: true,
          })),
          providers: ['Fiex'],
        }
      : undefined,
}
