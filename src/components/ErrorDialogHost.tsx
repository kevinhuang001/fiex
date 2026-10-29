import { useSyncExternalStore } from 'react'
import { ExplorerErrorDialog } from './ExplorerErrorDialog'
import { activeError, dismissError, reportError, subscribeErrors } from '@/lib/errorCenter'

/** Mount once per window. A navigation reset cannot dismiss unrelated errors. */
export function ErrorDialogHost() {
  const error = useSyncExternalStore(subscribeErrors, activeError, () => null)
  return (
    <ExplorerErrorDialog
      error={error?.message ?? ''}
      onClose={() => { if (error) dismissError(error.id) }}
      onError={(cause) => reportError(cause, 'error-dialog')}
    />
  )
}
