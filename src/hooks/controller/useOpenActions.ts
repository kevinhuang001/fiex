import type { Explorer } from '@/hooks/useExplorer'
import { call } from '@/lib/api'
import { pluginCatalog } from '@/lib/plugins'
import { runBeforeOpenHooks } from '@/lib/pluginHooks'
import type { Entry, Ide, Preferences, Tag } from '@/lib/types'
import type { Dispatch, SetStateAction } from 'react'
import type { Notify, Run } from './types'

/** Opening and previewing entries: folders, files (through before-open plugin
 *  hooks), remote downloads, the Trash, the Applications folder, terminals,
 *  IDEs and tag editing. */
export function useOpenActions({
  x,
  prefs,
  run,
  notify,
  writable,
  first,
  ides,
  isRemotePath,
  tagPaths,
  setTagPaths,
  setTagInitial,
  setTagOpen,
  setPreview,
  setTrashIssue,
  trashPath,
}: {
  x: Explorer
  prefs: Preferences
  run: Run
  notify: Notify
  writable: () => boolean
  first: Entry | undefined
  ides: Ide[]
  isRemotePath: (path: string) => boolean
  tagPaths: string[]
  setTagPaths: Dispatch<SetStateAction<string[]>>
  setTagInitial: Dispatch<SetStateAction<Tag[]>>
  setTagOpen: Dispatch<SetStateAction<boolean>>
  setPreview: Dispatch<SetStateAction<Entry | null>>
  setTrashIssue: Dispatch<SetStateAction<boolean>>
  trashPath: string | null
}) {
  const open = (e: Entry, pane = x) => {
    if (e.isDir && e.extension !== 'app') {
      setPreview(null)
      pane.navigate(e.path)
      return
    }
    void run('正在打开…', async () => {
      const plugins = await pluginCatalog()
      const outcome = await runBeforeOpenHooks(plugins, e)
      if (outcome.preview) {
        setPreview(e)
        return
      }
      if (outcome.handled) {
        pane.reload()
        return
      }
      await call('open_path', { path: e.path })
    })
  }
  const previewEntry = (entry: Entry, pane = x) => {
    if (!isRemotePath(entry.path)) {
      setPreview(entry)
      return
    }
    void run('正在下载…', async () => {
      await call('remote_download', { path: entry.path })
      setPreview(entry)
      pane.reload()
    })
  }
  const openTrash = async () => {
    if (!trashPath) {
      void run('正在打开废纸篓…', () => call('open_trash'))
      return
    }
    try {
      await call('list_directory', {
        path: trashPath,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
      })
      setPreview(null)
      x.navigate(trashPath)
    } catch (error) {
      const message = String(error)
      if (/permitted|permission|权限/i.test(message)) setTrashIssue(true)
      else x.setError(message)
    }
  }
  const openApplications = () => void run('正在打开应用程序…', () => call('open_applications'))
  const terminal = (id = prefs.terminal) => {
    if (id === 'builtin') {
      const cwd = first?.isDir ? first.path : x.path
      window.dispatchEvent(
        new CustomEvent('fiex:open-builtin-terminal', {
          detail: { cwd: isRemotePath(cwd) ? '' : cwd },
        }),
      )
      return
    }
    if (!writable()) return
    void run('正在打开终端…', () =>
      call('open_terminal', { path: first?.isDir ? first.path : x.path, id }),
    )
  }
  const openIde = (path = first?.path || x.path) => {
    if (!writable()) return
    if (!path) return
    const ide = ides.find((item) => item.id === prefs.ide) || ides[0]
    if (!ide) {
      x.setError('没有检测到可用的 IDE')
      return
    }
    void run(`正在用 ${ide.name} 打开…`, () => call('open_in_ide', { path, id: ide.id }))
  }
  const openTags = (paths: string[]) => {
    if (!writable()) return
    const wanted = new Set(paths)
    const initial = new Map<string, Tag>()
    for (const entry of x.entries) {
      if (!wanted.has(entry.path)) continue
      for (const tag of entry.tags) initial.set(`${tag.name}\u0000${tag.color}`, tag)
    }
    setTagPaths(paths)
    setTagInitial([...initial.values()])
    setTagOpen(true)
  }
  const applyTags = async (tags: Tag[]) => {
    if (!writable()) return
    try {
      await call('set_tags', { paths: tagPaths, tags })
      setTagOpen(false)
      x.reload()
      notify('标签已更新')
    } catch (error) {
      x.setError(String(error))
    }
  }
  return {
    open,
    previewEntry,
    openTrash,
    openApplications,
    terminal,
    openIde,
    openTags,
    applyTags,
  }
}
