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
  const run = async <T>(label: string, fn: () => Promise<T>): Promise<T | undefined> => {
    if (running.current) return
    running.current = true
    setBusy(label)
    x.setError('')
    try {
      const result = await fn()
      x.reload()
      return result
    } catch (e) {
      x.setError(String(e))
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
  const recycle = (confirm = true) => {
    if (!writable()) return
    if (!x.selected.length) return
    const paths = [...x.selected]
    if (inTrash) {
      deletePermanently(paths)
      return
    }
    if (!confirm) {
      void run('正在移到废纸篓…', () => moveToTrash(paths))
      return
    }
    setDialog({
      title: `将 ${paths.length} 个项目移到废纸篓？`,
      submit: '移到废纸篓',
      danger: true,
      action: () => moveToTrash(paths),
    })
  }
  const deletePermanently = (paths: string[]) => {
    if (!paths.length) return
    setDialog({
      title: `彻底删除 ${paths.length} 个项目？`,
      submit: '彻底删除',
      danger: true,
      action: async () => {
        await run('正在彻底删除…', () => call<number>('delete_permanently', { paths }))
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
        const removed = await run('正在清空废纸篓…', () => call<number>('empty_trash'))
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
    const result = await run(moving ? '正在移动…' : '正在复制…', () =>
      call<OperationResult>('transfer', {
        paths,
        destination,
        moving,
        keepBoth: false,
        ignores: prefs.copyIgnore,
        resolutions,
      }),
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
    void run('正在创建副本…', async () => {
      handleResult(
        await call<OperationResult>('transfer', {
          paths: x.selected,
          destination: x.path,
          moving: false,
          keepBoth: true,
          ignores: prefs.copyIgnore,
        }),
      )
    })
  }
  const compress = (format: ArchiveFormat) => {
    if (!writable()) return
    if (!first) return
    setDialog({
      title: `压缩为 ${format.name}`,
      label: '压缩包名称',
      value: (chosen.length === 1 ? first.name : 'Archive') + format.ext,
      submit: '压缩',
      action: async (value) => {
        let name = value.trim()
        if (!name.toLowerCase().endsWith(format.ext)) name += format.ext
        if (!name || name === '..' || /[\\/]/.test(name)) throw new Error('请输入有效的文件名')
        await call('compress_archive', {
          paths: x.selected,
          destination: join(x.path, name),
          format: format.id,
          ignores: prefs.compressIgnore,
        })
        notify('压缩完成')
        x.reload()
      },
    })
  }
  const extract = () => {
    if (!writable()) return
    if (first)
      void run('正在解压…', async () => {
        const path = await call<string>('extract_archive', { path: first.path })
        notify(`已解压到 ${basename(path)}`)
      })
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
