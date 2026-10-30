import { listen } from '@tauri-apps/api/event'
import { useEffect } from 'react'
import { t } from '@/i18n'
import { native } from '@/lib/api'
import { listActivity, recordActivity } from '@/hooks/useActivity'

interface CacheChange {
  path: string
  state: 'cached' | 'uncached' | 'uploaded' | 'conflict' | 'failed'
  detail?: string
}

/** Failed sync steps and accepted uploads are recorded; refreshes,
 *  invalidations and conflicts stay on the listing badges and menus. The
 *  listing refreshes through `remote-sync`; this never opens the panel. */
export function useRemoteActivity() {
  useEffect(() => {
    if (!native) return
    const stop = listen<{ connection: string; name?: string; changes: CacheChange[] }>(
      'remote-cache-state',
      (event) => {
        const failed = event.payload.changes.filter((change) => change.state === 'failed')
        if (failed.length) {
          const detail = failed
            .map((change) => `${change.path}: ${change.detail || ''}`.trim())
            .join('\n')
          const label = event.payload.name ? t('{0} 同步失败', event.payload.name) : t('同步失败')
          const duplicate = listActivity().some(
            (item) =>
              item.kind === 'sync' &&
              item.status === 'error' &&
              item.label === label &&
              item.detail === detail &&
              Date.now() - (item.finishedAt ?? item.startedAt) < 30 * 60 * 1000,
          )
          if (!duplicate) recordActivity(label, detail, 'sync', 'error')
        }
        const uploaded = event.payload.changes.filter((change) => change.state === 'uploaded')
        if (uploaded.length) {
          const detail =
            uploaded.length === 1
              ? uploaded[0].path
              : `${t('{0} 个项目', uploaded.length)} · ${uploaded
                  .slice(0, 2)
                  .map((change) => change.path)
                  .join('、')}${uploaded.length > 2 ? '…' : ''}`
          recordActivity(t('缓存已上传'), detail)
        }
      },
    )
    return () => {
      void stop.then((unlisten) => unlisten())
    }
  }, [])
}
