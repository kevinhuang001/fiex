import type { Dispatch, SetStateAction } from 'react'
import type { Run, FileOperationRuntimeServices } from '../types'

export interface RunnerPaneState {
  path: string
  selected: string[]
  setError: (message: string) => void
}

export interface RunnerLock {
  current: boolean
}

/** Builds the serialized operation runner from app-provided side effects. */
export function createRunner({
  x,
  running,
  setBusy,
  runtime,
  reportPermissionError,
}: {
  x: RunnerPaneState
  running: RunnerLock
  setBusy: Dispatch<SetStateAction<string>>
  runtime: FileOperationRuntimeServices
  reportPermissionError: (message: string, context: string) => boolean
}): Run {
  return async (
    label,
    fn,
    detail,
    cancellable = false,
    activityKind,
    onActivity,
    quiet = false,
    hold,
    resume,
    affectedDirectories,
  ) => {
    if (running.current) return
    running.current = true
    setBusy(label)
    x.setError('')
    if (cancellable) await runtime.beginOperation().catch(() => {})
    const activity =
      resume ??
      (activityKind
        ? runtime.beginActivity(
            label,
            detail,
            cancellable ? () => void runtime.cancelOperation().catch(() => {}) : undefined,
            activityKind,
            undefined,
            !quiet,
          )
        : undefined)
    if (resume)
      runtime.updateActivity(resume, {
        ...(detail === undefined ? {} : { detail }),
        indeterminate: false,
      })
    if (activity) onActivity?.(activity)
    try {
      const result = await fn()
      if (activity && !hold?.value) runtime.finishActivity(activity)
      if (affectedDirectories) runtime.requestDirectoryRefresh(affectedDirectories(result))
      return result
    } catch (error) {
      const message = String(error)
      if (message.startsWith('Cancelled:')) {
        if (affectedDirectories) runtime.requestDirectoryRefresh(affectedDirectories(undefined))
      } else {
        const location = detail?.trim() || (x.selected.length ? x.selected.join('\n') : x.path)
        if (reportPermissionError(message, `${label}\n${location}`)) x.setError('')
        else x.setError(message)
        if (affectedDirectories) runtime.requestDirectoryRefresh(affectedDirectories(undefined))
      }
      if (activity) runtime.finishActivity(activity, message)
      return undefined
    } finally {
      running.current = false
      setBusy('')
    }
  }
}
