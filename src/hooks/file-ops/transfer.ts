import { t } from '@/i18n'
import { call } from '@/lib/api'
import type { ConflictResolution, OperationResult } from '@/lib/types'
import { progressChannel } from '@/hooks/useActivity'
import type { FileOpsContext } from './types'

/** Copying, moving and duplicating entries, including conflict resolution. */
export function transferActions({
  x,
  notify,
  writable,
  conflict,
  setConflict,
  setClipboard,
  run,
  handleResult,
}: FileOpsContext) {
  const executeTransfer = async (
    paths: string[],
    destination: string,
    moving: boolean,
    resolutions: ConflictResolution[] = [],
  ) => {
    const shown = paths.slice(0, 5)
    const source = `${shown.join('\n')}${
      paths.length > shown.length ? t('\n… 另有 {0} 项', paths.length - shown.length) : ''
    }`
    let onProgress: ReturnType<typeof progressChannel> | undefined
    const result = await run(
      moving ? t('正在移动…') : t('正在复制…'),
      () =>
        call<OperationResult>('transfer', {
          paths,
          destination,
          moving,
          keepBoth: false,
          // Copy and move always take everything; browsing filters stay visual.
          ignores: [],
          resolutions,
          onProgress,
        }),
      t('{0}来源：\n{1}\n\n目标：\n{2}', moving ? t('移动') : t('复制'), source, destination),
      true,
      'transfer',
      (id) => {
        onProgress = progressChannel(id)
      },
    )
    if (!result) return
    if (result.conflicts?.length) {
      setConflict({ paths, destination, moving, items: result.conflicts })
      return
    }
    setConflict(null)
    if (moving) setClipboard(null)
    try {
      handleResult(result)
    } catch (error) {
      x.setError(String(error))
    }
  }
  const transfer = (paths: string[], destination: string, moving: boolean) => {
    if (!writable() || !paths.length || conflict) return
    void executeTransfer(paths, destination, moving)
  }
  const resolveConflict = async (resolutions: ConflictResolution[]) => {
    if (conflict)
      await executeTransfer(conflict.paths, conflict.destination, conflict.moving, resolutions)
  }
  const copy = (moving: boolean) => {
    if (moving && !writable()) return
    if (x.selected.length) {
      setClipboard({ paths: [...x.selected], moving })
      notify(moving ? t('已剪切') : t('已复制'))
    }
  }
  const duplicate = () => {
    if (!writable()) return
    let onProgress: ReturnType<typeof progressChannel> | undefined
    void run(
      t('正在创建副本…'),
      async () => {
        handleResult(
          await call<OperationResult>('transfer', {
            paths: x.selected,
            destination: x.path,
            moving: false,
            keepBoth: true,
            ignores: [],
            onProgress,
          }),
        )
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
