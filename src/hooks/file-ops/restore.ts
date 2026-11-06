import { t } from '@/i18n'
import { call } from '@/lib/api'
import type { FileOpsContext } from './types'

/** Putting Trash contents back and undoing the last file operation. */
export function restoreActions({ x, notify, run }: FileOpsContext) {
  const restoreFromTrash = (paths: string[]) => {
    if (!paths.length) return
    let fallbackDirectory: string | null = null
    let restoreStarted = false
    void run(
      t('正在复原…'),
      async () => {
        const needsFolder = await call<boolean>('trash_restore_needs_folder', { paths })
        const fallback = needsFolder ? await call<string | null>('pick_folder', {}) : null
        if (needsFolder && !fallback) return
        fallbackDirectory = fallback
        restoreStarted = true
        const restored = await call<number>('restore_from_trash', { paths, fallback })
        if (restored === undefined) return
        notify(t('已复原 {0} 个项目', restored))
        x.setSelected([])
      },
      undefined,
      true,
      undefined,
      undefined,
      false,
      undefined,
      undefined,
      (restored) =>
        (restored === undefined && restoreStarted) || (restored !== undefined && restored > 0)
          ? [x.path, ...(fallbackDirectory ? [fallbackDirectory] : [])]
          : [],
    )
  }
  const undo = () => {
    void run(
      t('正在撤销…'),
      async () => {
        await call('undo')
        notify(t('已撤销上一次文件操作'))
      },
      undefined,
      false,
      undefined,
      undefined,
      false,
      undefined,
      undefined,
      () => [x.path],
    )
  }
  return { restoreFromTrash, undo }
}
