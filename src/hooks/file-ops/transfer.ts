import { t } from '@/i18n'
import { call } from '@/lib/api'
import { reportFilesystemPermissionError } from '@/lib/errorCenter'
import { parentDirectoryOf } from '@/lib/listing'
import type { ConflictResolution, Entry } from '@/lib/types'
import { progressChannel, quietProgressChannel, updateActivity } from '@/hooks/useActivity'
import type { FileOpsContext } from './types'

export function canDuplicateEntries(
  entries: Array<Entry | undefined>,
  allowedOps: string[],
): boolean {
  if (!entries.length || entries.some((entry) => !entry)) return false
  const needsDirectory = entries.some((entry) => entry?.isDir)
  const needsFile = entries.some((entry) => !entry?.isDir)
  return (
    (!needsDirectory || allowedOps.includes('mkdir')) &&
    (!needsFile || allowedOps.includes('write'))
  )
}

/** Copying, moving and duplicating entries, including conflict resolution. */
export function transferActions({ x, conflict, setConflict, setClipboard, run }: FileOpsContext) {
  const executeTransfer = async (
    paths: string[],
    destination: string,
    moving: boolean,
    resolutions: ConflictResolution[] = [],
    /** Continue the activity row a pending conflict dialog is holding. */
    resume?: string,
  ) => {
    const sourceDirectories = moving
      ? await Promise.all(paths.map((path) => call('file_parent', { path }).catch(() => undefined)))
      : []
    const affectedSourceDirectories = moving
      ? paths.map((path, index) => sourceDirectories[index] ?? parentDirectoryOf(path))
      : []
    const actionablePaths = moving
      ? paths.filter(
          (path, index) =>
            path !== destination &&
            (sourceDirectories[index] ?? parentDirectoryOf(path)) !== destination,
        )
      : paths
    if (!actionablePaths.length) return

    const shown = actionablePaths.slice(0, 5)
    const source = `${shown.join('\n')}${
      actionablePaths.length > shown.length
        ? t('\n… 另有 {0} 项', actionablePaths.length - shown.length)
        : ''
    }`
    let onProgress: ReturnType<typeof progressChannel> = quietProgressChannel()
    let activityId = resume
    // A conflict dialog keeps this row running; `hold` stops `run` from
    // finishing it and the resolution reuses the same id instead of adding a
    // second "copy" record to the status centre.
    const hold = { value: false }
    const result = await run(
      moving ? t('正在移动…') : t('正在复制…'),
      async () => {
        const next = await call('transfer', {
          paths: actionablePaths,
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
        onProgress = progressChannel(id) ?? quietProgressChannel()
      },
      false,
      hold,
      resume,
      (next) => {
        if (next === undefined || next.paths.length > 0)
          return [...new Set([destination, ...affectedSourceDirectories])]
        return []
      },
    )
    if (!result) return
    if (result.conflicts?.length) {
      if (activityId)
        updateActivity(activityId, { detail: t('等待解决冲突…'), indeterminate: true })
      setConflict({
        paths: actionablePaths,
        destination,
        moving,
        items: result.conflicts,
        activityId,
      })
      return
    }
    setConflict(null)
    if (moving) setClipboard(null)
    if (result.errors.length) {
      const message = t('{0} 项已完成。{1}', result.paths.length, result.errors.join('\n'))
      const context = t(
        '{0}来源：\n{1}\n\n目标：\n{2}',
        moving ? t('移动') : t('复制'),
        source,
        destination,
      )
      if (!reportFilesystemPermissionError(message, context)) x.setError(message)
    }
  }
  const transfer = (paths: string[], destination: string, moving: boolean) => {
    if (!paths.length || conflict) return
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
    if (paths.length) setClipboard({ paths: [...paths], moving })
  }
  const duplicate = (paths = x.selected) => {
    const entries = paths.map((path) => x.entries.find((entry) => entry.path === path))
    if (!canDuplicateEntries(entries, x.directoryAllowedOps)) {
      x.setError(t('此位置不支持该操作'))
      return
    }
    let onProgress: ReturnType<typeof progressChannel> = quietProgressChannel()
    void run(
      t('正在创建副本…'),
      async () => {
        const result = await call('transfer', {
          paths,
          destination: x.path,
          moving: false,
          keepBoth: true,
          ignores: [],
          onProgress,
        })
        if (result.errors.length)
          throw new Error(t('{0} 项已完成。{1}', result.paths.length, result.errors.join('\n')))
        return result
      },
      undefined,
      true,
      'transfer',
      (id) => {
        onProgress = progressChannel(id) ?? quietProgressChannel()
      },
      false,
      undefined,
      undefined,
      (result) => (result === undefined || result.paths.length > 0 ? [x.path] : []),
    )
  }
  return { executeTransfer, transfer, resolveConflict, copy, duplicate }
}
