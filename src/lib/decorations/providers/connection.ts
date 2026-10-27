import { t } from '@/i18n'
import type { DecorationBadge } from '@/lib/types'
import type { DecorationProvider } from '../types'

export const connectionDecorationProvider: DecorationProvider = {
  id: 'builtin:connection-status',
  priority: 10,
  resolve(entry) {
    if (!entry.sync) return
    const states = {
      synced: { text: t('已缓存'), icon: 'cloud-check', color: 'green' },
      cloud: { text: t('未缓存'), icon: 'cloud-download', color: 'gray' },
      pinned: { text: t('始终保留在本地'), icon: 'pin', color: 'purple' },
      pending: { text: t('等待上传'), icon: 'cloud-check', color: 'orange' },
      conflict: { text: t('同步冲突'), icon: 'cloud-check', color: 'red' },
    } as const
    const state = states[entry.sync]
    const badges: DecorationBadge[] = [
      {
        ...state,
        tooltip: state.text,
        status: entry.sync,
      },
    ]
    if (entry.uploaded && (entry.hasCache || entry.pinned))
      badges.push({
        text: t('已上传'),
        tooltip: t('已上传'),
        color: 'blue',
        icon: 'cloud-upload',
        status: 'uploaded',
      })
    return { path: entry.path, badges, providers: ['Fiex'] }
  },
}
