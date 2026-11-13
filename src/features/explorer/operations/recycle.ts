import { t } from '@/i18n'
import { isWindows } from '@/lib/platform'
import { call } from '@/lib/api'
import type { FileOpsContext } from './types'

/** Moving entries to the Trash and deleting them for good. */
export function recycleActions({ x, inTrash, setDialog, run, activity }: FileOpsContext) {
  const { progressChannel, quietProgressChannel } = activity
  const moveToTrash = async (
    paths: string[],
    onProgress: ReturnType<typeof progressChannel> = quietProgressChannel(),
  ) => {
    return call('recycle', { paths, onProgress })
  }
  // Moving to the Trash is reversible, so it never asks for confirmation.
  const recycle = (selection = x.selected) => {
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
      deletePermanently(paths)
      return
    }
    let onProgress: ReturnType<typeof progressChannel> = quietProgressChannel()
    void (async () => {
      const result = await run(
        t(isWindows() ? '正在移到回收站…' : '正在移到废纸篓…'),
        () => moveToTrash(paths, onProgress),
        t(
          isWindows() ? '删除来源：\n{0}\n\n目标：\n回收站' : '删除来源：\n{0}\n\n目标：\n废纸篓',
          source,
        ),
        true,
        'delete',
        (id) => {
          onProgress = progressChannel(id) ?? quietProgressChannel()
        },
        false,
        undefined,
        undefined,
        (result) =>
          result === undefined || result.paths.length > 0
            ? ['trash://', 'recyclebin://', x.path]
            : [],
      )
      if (result?.paths.length) x.setSelected([])
      if (result?.errors.length)
        x.setError(t('{0} 项已完成。{1}', result.paths.length, result.errors.join('\n')))
    })()
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
          () => call('delete_permanently', { paths }),
          t('删除：\n{0}', source),
          true,
          'delete',
          undefined,
          false,
          undefined,
          undefined,
          (removed) => (removed === undefined || removed > 0 ? [x.path] : []),
        )
        x.setSelected([])
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
          () => call('empty_trash'),
          t(isWindows() ? '删除：回收站中的所有项目' : '删除：废纸篓中的所有项目'),
          true,
          'delete',
          undefined,
          false,
          undefined,
          undefined,
          (removed) =>
            removed === undefined || removed > 0 ? [x.path, 'trash://', 'recyclebin://'] : [],
        )
        if (removed === undefined) return
        x.setSelected([])
      },
    })
  }
  return { moveToTrash, recycle, deletePermanently, emptyTrash }
}
