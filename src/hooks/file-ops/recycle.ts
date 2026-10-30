import { t } from '@/i18n'
import { isWindows } from '@/lib/platform'
import { call } from '@/lib/api'
import type { OperationResult } from '@/lib/types'
import { progressChannel } from '@/hooks/useActivity'
import type { FileOpsContext } from './types'

/** Moving entries to the Trash and deleting them for good. */
export function recycleActions({ x, writable, inTrash, setDialog, run }: FileOpsContext) {
  const moveToTrash = async (paths: string[], onProgress?: ReturnType<typeof progressChannel>) => {
    const result = await call<OperationResult>('recycle', { paths, onProgress })
    if (result.errors.length)
      throw new Error(t('{0} 项已完成。{1}', result.paths.length, result.errors.join('\n')))
    x.setSelected([])
    x.reload()
  }
  // Moving to the Trash is reversible, so it never asks for confirmation.
  const recycle = (selection = x.selected) => {
    if (!writable()) return
    if (!selection.length) return
    const paths = [...selection]
    const shown = paths.slice(0, 5)
    const source = `${shown.join('\n')}${
      paths.length > shown.length ? t('\n… 另有 {0} 项', paths.length - shown.length) : ''
    }`
    if (inTrash) {
      deletePermanently(paths)
      return
    }
    if (paths.some((path) => path.startsWith('applications://'))) {
      // The Applications store lives at a virtual URI; Core resolves it to the
      // real store path and moves it to the platform Trash.
      void run(
        t(isWindows() ? '正在移到回收站…' : '正在移到废纸篓…'),
        async () => {
          await call('trash_application_entries', { paths })
          x.setSelected([])
          x.reload()
        },
        t(
          isWindows() ? '删除来源：\n{0}\n\n目标：\n回收站' : '删除来源：\n{0}\n\n目标：\n废纸篓',
          source,
        ),
        true,
        'delete',
      )
      return
    }
    let onProgress: ReturnType<typeof progressChannel> | undefined
    void run(
      t(isWindows() ? '正在移到回收站…' : '正在移到废纸篓…'),
      () => moveToTrash(paths, onProgress),
      t(
        isWindows() ? '删除来源：\n{0}\n\n目标：\n回收站' : '删除来源：\n{0}\n\n目标：\n废纸篓',
        source,
      ),
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
        x.reload()
      },
    })
  }
  const emptyTrash = () => {
    setDialog({
      title: t(isWindows() ? '清空回收站？' : '清空废纸篓？'),
      submit: t(isWindows() ? '清空回收站' : '清空废纸篓'),
      danger: true,
      action: async () => {
        const removed = await run(
          t(isWindows() ? '正在清空回收站…' : '正在清空废纸篓…'),
          () => call<number>('empty_trash'),
          t(isWindows() ? '删除：回收站中的所有项目' : '删除：废纸篓中的所有项目'),
          true,
          'delete',
        )
        if (removed === undefined) return
        x.setSelected([])
      },
    })
  }
  return { moveToTrash, recycle, deletePermanently, emptyTrash }
}
