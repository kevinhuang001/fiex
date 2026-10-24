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
      paths.length > shown.length ? `\n… 另有 ${paths.length - shown.length} 项` : ''
    }`
    let onProgress: ReturnType<typeof progressChannel> | undefined
    void run(
      '正在移到废纸篓…',
      () => moveToTrash(paths, onProgress),
      `删除来源：\n${source}\n\n目标：\n废纸篓`,
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
      title: `彻底删除 ${paths.length} 个项目？`,
      submit: '彻底删除',
      danger: true,
      action: async () => {
        const shown = paths.slice(0, 5)
        const source = `${shown.join('\n')}${
          paths.length > shown.length ? `\n… 另有 ${paths.length - shown.length} 项` : ''
        }`
        await run(
          '正在彻底删除…',
          () => call<number>('delete_permanently', { paths }),
          `删除：\n${source}`,
          true,
          'delete',
        )
        x.setSelected([])
      },
    })
  }
  const emptyTrash = () => {
    setDialog({
      title: '清空废纸篓？',
      submit: '清空废纸篓',
      danger: true,
      action: async () => {
        const removed = await run(
          '正在清空废纸篓…',
          () => call<number>('empty_trash'),
          '删除：废纸篓中的所有项目',
          true,
          'delete',
        )
        if (removed === undefined) return
        x.setSelected([])
        notify(removed > 0 ? `已清空 ${removed} 个项目` : '废纸篓已经是空的')
      },
    })
  }
  return { moveToTrash, recycle, deletePermanently, emptyTrash }
}
