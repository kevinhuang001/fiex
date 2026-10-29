import { useEffect } from 'react'
import { ExplorerErrorDialog } from '@/components/ExplorerErrorDialog'
import { dismissError, receiveCoreErrors, reportError, useCurrentError } from '@/lib/errorCenter'

/** Single global error modal. Only actionable failures are enqueued; background
 * diagnostics stay in LogCenter without interrupting the user. */
export function ErrorDialogHost({
  legacyError,
  clearLegacy,
}: { legacyError: string; clearLegacy: () => void }) {
  const current = useCurrentError()
  useEffect(() => {
    if (legacyError) reportError(legacyError, 'ui.action', 'explorer')
  }, [legacyError])
  useEffect(() => {
    void receiveCoreErrors().catch(() => {})
    const timer = window.setInterval(() => {
      if (!document.hidden) void receiveCoreErrors().catch(() => {})
    }, 1400)
    const failedStateWrite = (event: Event) =>
      reportError(String((event as CustomEvent<string>).detail), 'state.write', 'state')
    window.addEventListener('fiex:state-error', failedStateWrite)
    return () => {
      window.clearInterval(timer)
      window.removeEventListener('fiex:state-error', failedStateWrite)
    }
  }, [])
  return <ExplorerErrorDialog
    error={current?.message || ''}
    onClose={() => { dismissError(); clearLegacy() }}
    onError={(message) => reportError(message, 'ui.action', 'dialog')}
  />
}
