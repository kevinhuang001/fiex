import { t } from '@/i18n'
import { call } from '@/lib/api'
import { beginActivity, finishActivity, updateActivity } from '@/hooks/useActivity'
import type { Explorer } from '@/hooks/useExplorer'
import { useRef } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import type { Run } from './types'

/** Serializes file operations: only one runs at a time and the busy label
 *  follows it, including the cancellable activity it registers. */
export function useRunner({
  x,
  setBusy,
}: {
  x: Explorer
  setBusy: Dispatch<SetStateAction<string>>
}) {
  const running = useRef(false)
  const run: Run = async (
    label,
    fn,
    detail,
    cancellable = false,
    activityKind,
    onActivity,
    quiet = false,
    hold,
    resume,
  ) => {
    if (running.current) return
    running.current = true
    setBusy(label)
    x.setError('')
    if (cancellable) {
      // Reset the shared backend flag first so a previous cancel cannot abort
      // the operation we are about to start.
      await call('begin_operation').catch(() => {})
    }
    const activity =
      resume ??
      (activityKind
        ? beginActivity(
            label,
            detail,
            cancellable
              ? () => {
                  void call('cancel_operation').catch(() => {})
                }
              : undefined,
            activityKind,
            undefined,
            !quiet,
          )
        : undefined)
    if (resume) updateActivity(resume, { detail, indeterminate: false })
    if (activity) onActivity?.(activity)
    try {
      const result = await fn()
      if (activity && !hold?.value) finishActivity(activity)
      x.reload()
      return result
    } catch (e) {
      const message = String(e)
      if (message.includes(t('已取消'))) {
        // Cancelling is the user's choice; the activity centre already says so.
        // Refresh because a stopped transfer may still have moved some files.
        x.reload()
      } else {
        x.setError(message)
      }
      if (activity) finishActivity(activity, message)
      return undefined
    } finally {
      running.current = false
      setBusy('')
    }
  }
  return { run }
}
