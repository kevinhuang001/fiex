import { call } from '@/lib/api'
import type { DialogState } from '@/lib/dialogs'
import { uniqueName } from '@/lib/files'
import type { OperationResult, TransferConflict, ConflictResolution } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'
import { useRef, useState } from 'react'
import type { Explorer } from './useExplorer'
import { beginActivity, finishActivity, progressChannel, type ActivityKind } from './useActivity'

export function useFileOperations({
  x,
  writable,
  notify,
  setDialog,
  trashPath,
}: {
  x: Explorer
  writable: () => boolean
  notify: (message: string) => void
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  trashPath: string | null
}) {
  // Inside the Trash, "move to Trash" would be a no-op, so the delete action
  // becomes permanent just like the system file manager.
  const inTrash = Boolean(trashPath && (x.path === trashPath || x.path.startsWith(trashPath + '/')))
  const [busy, setBusy] = useState('')
  const [conflict, setConflict] = useState<{
    paths: string[]
    destination: string
    moving: boolean
    items: TransferConflict[]
  } | null>(null)
  const running = useRef(false)
  const [clipboard, setClipboard] = useState<{ paths: string[]; moving: boolean } | null>(null)
  const run = async <T>(
    label: string,
    fn: () => Promise<T>,
    detail?: string,
    cancellable = false,
    activityKind?: ActivityKind,
    onActivity?: (id: string) => void,
  ): Promise<T | undefined> => {
    if (running.current) return
    running.current = true
    setBusy(label)
    x.setError('')
    if (cancellable) {
      // Reset the shared backend flag first so a previous cancel cannot abort
      // the operation we are about to start.
      await call('begin_operation').catch(() => {})
    }
    const activity = activityKind
      ? beginActivity(
          label,
          detail,
          cancellable
            ? () => {
                void call('cancel_operation').catch(() => {})
              }
            : undefined,
          activityKind,
        )
      : undefined
    if (activity) onActivity?.(activity)
    try {
      const result = await fn()
      if (activity) finishActivity(activity)
      x.reload()
      return result
    } catch (e) {
      const message = String(e)
      if (message.includes('已取消')) {
        // Cancelling is the user's choice; the activity centre already says so.
        // Refresh because a stopped transfer may still have moved some files.
        x.reload()
      } else {
        x.setError(message)
      }
      if (activity) finishActivity(activity, message)
      return undefined
    } finally {
      running.current = false
      setBusy('')
    }
  }
  const handleResult = (result: OperationResult) => {
    if (result.errors.length)
      throw new Error(`${result.paths.length} 项已完成。${result.errors.join('\n')}`)
    notify(
      result.skipped
        ? `${result.paths.length} 项操作完成，已跳过 ${result.skipped} 项`
        : `${result.paths.length} 项操作完成`,
    )
  }
  const create = async (folder: boolean) => {
    if (!writable()) return
    const preferred = folder ? '未命名文件夹' : '未命名.txt'
    const name = uniqueName(
      x.entries.map((e) => e.name),
      preferred,
    )
    const path = await run(folder ? '正在新建文件夹…' : '正在新建文件…', () =>
      call<string>('create_entry', { path: x.path, name, folder }),
    )
    if (!path) return
    if (x.query) x.setQuery('')
    x.beginRename(path)
  }
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
  const executeTransfer = async (
    paths: string[],
    destination: string,
    moving: boolean,
    resolutions: ConflictResolution[] = [],
  ) => {
    const shown = paths.slice(0, 5)
    const source = `${shown.join('\n')}${
      paths.length > shown.length ? `\n… 另有 ${paths.length - shown.length} 项` : ''
    }`
    let onProgress: ReturnType<typeof progressChannel> | undefined
    const result = await run(
      moving ? '正在移动…' : '正在复制…',
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
      `${moving ? '移动' : '复制'}来源：\n${source}\n\n目标：\n${destination}`,
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
      notify(moving ? '已剪切' : '已复制')
    }
  }
  const duplicate = () => {
    if (!writable()) return
    let onProgress: ReturnType<typeof progressChannel> | undefined
    void run(
      '正在创建副本…',
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
  const undo = () => {
    if (!writable()) return
    void run('正在撤销…', async () => {
      await call('undo')
      notify('已撤销上一次文件操作')
    })
  }
  return {
    busy,
    conflict,
    setConflict,
    resolveConflict,
    clipboard,
    setClipboard,
    run,
    create,
    recycle,
    deletePermanently,
    restoreFromTrash,
    emptyTrash,
    inTrash,
    transfer,
    copy,
    duplicate,
    undo,
  }
}
