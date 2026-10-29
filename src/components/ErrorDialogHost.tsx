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
    const refresh = async () => {
      try {
        const entries = await call<Array<{ source: string; code: string; message: string }>>('errors_pending')
        if (alive) {
          for (const entry of entries) reportError(entry.message, `${entry.source}:${entry.code}`)
        }
      } catch {
        // A failed diagnostic poll must not create another diagnostic loop.
      }
    }
    void refresh()
    const interval = window.setInterval(() => void refresh(), 1500)
    return () => { alive = false; window.clearInterval(interval) }
  }, [])
  return (
    <ExplorerErrorDialog
      error={error?.message ?? ''}
      onClose={() => { if (error) dismissError(error.id) }}
      onError={(cause) => reportError(cause, 'error-dialog')}
    />
  )
}
