import { t } from '@/i18n'
import { call } from '@/lib/api'
import type { OperationResult } from '@/lib/types'
import { progressChannel } from '@/hooks/useActivity'
import type { FileOpsContext } from './types'

/** Moving entries to the Trash and deleting them for good. */
export function recycleActions({
  x,
  notify,
  writable,
  inTrash,
  setDialog,
  run,
  handleResult,
}: FileOpsContext) {
  const moveToTrash = async (paths: string[], onProgress?: ReturnType<typeof progressChannel>) => {
    handleResult(await call<OperationResult>('recycle', { paths, onProgress }))
    x.setSelected([])
    x.reload()
  }
  // Moving to the Trash is reversible, so it never asks for confirmation.
  const recycle = () => {
    if (!writable()) return
    if (!x.selected.length) return
    const paths = [...x.selected]
    if (inTrash) {
      deletePermanently(paths)
      return
    }
    const shown = paths.slice(0, 5)
    const source = `${shown.join('\n')}${
      paths.length > shown.length ? t('\n… 另有 {0} 项', paths.length - shown.length) : ''
    }`
    let onProgress: ReturnType<typeof progressChannel> | undefined
    void run(
      t('正在移到废纸篓…'),
      () => moveToTrash(paths, onProgress),
      t('删除来源：\n{0}\n\n目标：\n废纸篓', source),
      true,
      'delete',
      (id) => {
        onProgress = progressChannel(id)
      },
    )
  }
  const deletePermanently = (paths: string[]) => {
    if (!paths.length) return
    setDialog({
      title: t('彻底删除 {0} 个项目？', paths.length),
      submit: t('彻底删除'),
      danger: true,
      action: async () => {
        const shown = paths.slice(0, 5)
        const source = `${shown.join('\n')}${
          paths.length > shown.length ? t('\n… 另有 {0} 项', paths.length - shown.length) : ''
        }`
        await run(
          t('正在彻底删除…'),
          () => call<number>('delete_permanently', { paths }),
          t('删除：\n{0}', source),
          true,
          'delete',
        )
        x.setSelected([])
      },
    })
  }
  const emptyTrash = () => {
    setDialog({
      title: t('清空废纸篓？'),
      submit: t('清空废纸篓'),
      danger: true,
      action: async () => {
        const removed = await run(
          t('正在清空废纸篓…'),
          () => call<number>('empty_trash'),
          t('删除：废纸篓中的所有项目'),
          true,
          'delete',
        )
        if (removed === undefined) return
        x.setSelected([])
        notify(removed > 0 ? t('已清空 {0} 个项目', removed) : t('废纸篓已经是空的'))
      },
    })
  }
  return { moveToTrash, recycle, deletePermanently, emptyTrash }
}
