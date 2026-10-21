import { call } from '@/lib/api'
import type { ArchiveFormat } from '@/lib/archives'
import type { DialogState } from '@/lib/dialogs'
import { basename, join, uniqueName } from '@/lib/files'
import type {
  Entry,
  OperationResult,
  Preferences,
  TransferConflict,
  ConflictResolution,
} from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'
import { useRef, useState } from 'react'
import type { Explorer } from './useExplorer'
import { beginActivity, finishActivity } from './useActivity'

export function useFileOperations({
  x,
  prefs,
  writable,
  notify,
  setDialog,
  chosen,
  first,
  trashPath,
}: {
  x: Explorer
  prefs: Preferences
  writable: () => boolean
  notify: (message: string) => void
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  chosen: Entry[]
  first: Entry | undefined
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
    const activity = beginActivity(
      label,
      detail,
      cancellable
        ? () => {
            void call('cancel_operation').catch(() => {})
          }
        : undefined,
    )
    try {
      const result = await fn()
      finishActivity(activity)
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
      finishActivity(activity, message)
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
  const moveToTrash = async (paths: string[]) => {
    handleResult(await call<OperationResult>('recycle', { paths }))
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
    void run('正在移到废纸篓…', () => moveToTrash(paths), `${paths.length} 个项目 → 废纸篓`, true)
  }
  const deletePermanently = (paths: string[]) => {
    if (!paths.length) return
    setDialog({
      title: `彻底删除 ${paths.length} 个项目？`,
      submit: '彻底删除',
      danger: true,
      action: async () => {
        await run(
          '正在彻底删除…',
          () => call<number>('delete_permanently', { paths }),
          undefined,
          true,
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
        const restored = await call<number>('restore_from_trash', { paths })
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
          undefined,
          true,
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
        }),
      `${paths.length} 项 → ${destination}`,
      true,
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
          }),
        )
      },
      undefined,
      true,
    )
  }
  const compress = (
    format: ArchiveFormat,
    options: { level?: number; password?: string; full?: boolean } = {},
  ) => {
    if (!writable()) return
    if (!first) return
    const base = chosen.length === 1 ? first.name : 'Archive'
    const name = uniqueName(
      x.entries.map((entry) => entry.name),
      base + format.ext,
    )
    void run(
      '正在压缩…',
      async () => {
        await call('compress_archive', {
          paths: x.selected,
          destination: join(x.path, name),
          format: format.id,
          ignores: options.full ? [] : prefs.compressIgnore,
          level: options.level ?? null,
          password: options.password || null,
        })
        notify(`已压缩为 ${name}`)
      },
      `${base} → ${name} · ${format.name}${options.password ? ' · AES-256' : ''}${
        options.full ? ' · 全量' : ''
      }`,
      true,
    )
  }
  const extract = () => {
    if (!writable()) return
    if (!first) return
    void call<string | null>('pick_folder', { default: x.path })
      .then((destination) => {
        if (!destination) return
        void run(
          '正在解压…',
          async () => {
            const path = await call<string>('extract_archive', {
              path: first.path,
              destination,
            })
            notify(`已解压到 ${basename(path)}`)
            x.reload()
          },
          `${basename(first.path)} → ${basename(destination)}`,
          true,
        )
      })
      .catch((error) => x.setError(String(error)))
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
    compress,
    extract,
    undo,
  }
}
