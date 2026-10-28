import { t } from '@/i18n'
import { call } from '@/lib/api'
import type { ConflictResolution, OperationResult } from '@/lib/types'
import { progressChannel, updateActivity } from '@/hooks/useActivity'
import type { FileOpsContext } from './types'

/** Copying, moving and duplicating entries, including conflict resolution. */
export function transferActions({
  x,
  writable,
  conflict,
  setConflict,
  setClipboard,
  run,
}: FileOpsContext) {
  const executeTransfer = async (
    paths: string[],
    destination: string,
    moving: boolean,
    resolutions: ConflictResolution[] = [],
    /** Continue the activity row a pending conflict dialog is holding. */
    resume?: string,
  ) => {
    const shown = paths.slice(0, 5)
    const source = `${shown.join('\n')}${
      paths.length > shown.length ? t('\n… 另有 {0} 项', paths.length - shown.length) : ''
    }`
    let onProgress: ReturnType<typeof progressChannel> | undefined
    let activityId = resume
    // A conflict dialog keeps this row running; `hold` stops `run` from
    // finishing it and the resolution reuses the same id instead of adding a
    // second "copy" record to the status centre.
    const hold = { value: false }
    const result = await run(
      moving ? t('正在移动…') : t('正在复制…'),
      async () => {
        const next = await call<OperationResult>('transfer', {
          paths,
          destination,
          moving,
          keepBoth: false,
          // Copy and move always take everything; browsing filters stay visual.
          ignores: [],
          resolutions,
          onProgress,
        })
        if (next.conflicts?.length) hold.value = true
        return next
      },
      t('{0}来源：\n{1}\n\n目标：\n{2}', moving ? t('移动') : t('复制'), source, destination),
      true,
      'transfer',
      (id) => {
        activityId = id
        onProgress = progressChannel(id)
      },
      false,
      hold,
      resume,
    )
    if (!result) return
    if (result.conflicts?.length) {
      if (activityId)
        updateActivity(activityId, { detail: t('等待解决冲突…'), indeterminate: true })
      setConflict({ paths, destination, moving, items: result.conflicts, activityId })
      return
    }
    setConflict(null)
    if (moving) setClipboard(null)
    if (result.errors.length)
      x.setError(t('{0} 项已完成。{1}', result.paths.length, result.errors.join('\n')))
  }
  const transfer = (paths: string[], destination: string, moving: boolean) => {
    if (!writable() || !paths.length || conflict) return
    void executeTransfer(paths, destination, moving)
  }
  const resolveConflict = async (resolutions: ConflictResolution[]) => {
    if (conflict)
      await executeTransfer(
        conflict.paths,
        conflict.destination,
        conflict.moving,
        resolutions,
        conflict.activityId,
      )
  }
  const copy = (moving: boolean, paths = x.selected) => {
    if (moving && !writable()) return
    if (paths.length) setClipboard({ paths: [...paths], moving })
  }
  const duplicate = (paths = x.selected) => {
    if (!writable()) return
    let onProgress: ReturnType<typeof progressChannel> | undefined
    void run(
      t('正在创建副本…'),
      async () => {
        const result = await call<OperationResult>('transfer', {
          paths,
          destination: x.path,
          moving: false,
          keepBoth: true,
          ignores: [],
          onProgress,
        })
        if (result.errors.length)
          throw new Error(t('{0} 项已完成。{1}', result.paths.length, result.errors.join('\n')))
      },
      undefined,
      true,
      'transfer',
      (id) => {
        onProgress = progressChannel(id)
      },
    )
  }
  return { executeTransfer, transfer, resolveConflict, copy, duplicate }
}
