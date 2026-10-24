import { call } from '@/lib/api'
import type { FileOpsContext } from './types'

/** Putting Trash contents back and undoing the last file operation. */
export function restoreActions({ x, notify, writable, run }: FileOpsContext) {
  const restoreFromTrash = (paths: string[]) => {
    if (!paths.length) return
    void run(
      '正在复原…',
      async () => {
        const needsFolder = await call<boolean>('trash_restore_needs_folder', { paths })
        const fallback = needsFolder ? await call<string | null>('pick_folder', {}) : null
        if (needsFolder && !fallback) return
        const restored = await call<number>('restore_from_trash', { paths, fallback })
        if (restored === undefined) return
        notify(`已复原 ${restored} 个项目`)
        x.setSelected([])
      },
      undefined,
      true,
    )
  }
  const undo = () => {
    if (!writable()) return
    void run('正在撤销…', async () => {
      await call('undo')
      notify('已撤销上一次文件操作')
    })
  }
  return { restoreFromTrash, undo }
}
