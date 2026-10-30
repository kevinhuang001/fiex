import { useEffect, useSyncExternalStore } from 'react'
import { call, native } from '@/lib/api'
import { ExplorerErrorDialog } from './ExplorerErrorDialog'
import { activeError, dismissError, reportError, subscribeErrors } from '@/lib/errorCenter'

/** Mount once per window. A navigation reset cannot dismiss unrelated errors. */
export function ErrorDialogHost() {
  const error = useSyncExternalStore(subscribeErrors, activeError, () => null)
  useEffect(() => {
    if (!native) return
    let alive = true
    let cursor = 0
    let polling = false
    const refresh = async () => {
      if (polling) return
      polling = true
      try {
        const entries = await call<
          Array<{ id: number; source: string; code: string; message: string }>
        >('errors_pending', { after: cursor })
        if (alive) {
          for (const entry of entries) {
            reportError(entry.message, `${entry.source}:${entry.code}`, `error:${entry.id}`)
            cursor = Math.max(cursor, entry.id)
          }
        }
      } catch {
        // A failed diagnostic poll must not create another diagnostic loop.
      } finally {
        polling = false
      }
    }
    void refresh()
    const interval = window.setInterval(() => void refresh(), 1500)
    return () => {
      alive = false
      window.clearInterval(interval)
    }
  }, [])
  return (
    <ExplorerErrorDialog
      error={error?.message ?? ''}
      diagnosticId={error?.diagnosticId}
      onClose={() => {
        if (error) dismissError(error.id)
      }}
      onError={(cause) => reportError(cause, 'error-dialog')}
    />
  )
}
