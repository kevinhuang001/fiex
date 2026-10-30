import { t } from '@/i18n'
import type { Explorer } from '@/hooks/useExplorer'
import { progressChannel } from '@/hooks/useActivity'
import { call } from '@/lib/api'
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
  trashPath: string | null
}) {
  const open = async (e: Entry, pane = x) => {
    if (
      pane.path.startsWith('trash://') ||
      pane.path.startsWith('recyclebin://') ||
      (trashPath && (pane.path === trashPath || pane.path.startsWith(trashPath + '/')))
    ) {
      if (e.isDir && !e.isSymlink && e.path.startsWith('trash://')) {
        pane.navigate(e.path)
        return
      }
      pane.setError(t('回收站项目请使用“放回原处”或“彻底删除”'))
      return
    }
    if (e.isDir && e.extension !== 'app') {
      setPreview(null)
      pane.navigate(e.path)
      return
    }
    const remote = isRemotePath(e.path)
    // The listing snapshot can be stale after an upload or eviction. Ask the
    // cache registry before deciding whether this open needs an activity.
    const cached = remote
      ? await call<boolean>('remote_cached', { path: e.path }).catch(() => Boolean(e.hasCache))
      : false
    let onProgress: ReturnType<typeof progressChannel> | undefined
    void run(
      // Cached files open directly; only a real cache miss gets a transfer
      // activity in the status centre.
      remote && !cached ? t('正在缓存…') : t('正在打开…'),
      async () => {
        const outcome = await runBeforeOpenHooks(e)
        if (outcome.preview) {
          setPreview(e)
          return
        }
        if (outcome.handled) {
          pane.reload()
          return
        }
        if (remote) await call('remote_open', { path: e.path, onProgress })
        else await call('open_path', { path: e.path })
      },
      e.name,
      false,
      remote && !cached ? 'transfer' : undefined,
      remote && !cached
        ? (id) => {
            onProgress = progressChannel(id)
          }
        : undefined,
    )
  }
  const previewEntry = (entry: Entry, pane = x) => {
    if (!isRemotePath(entry.path)) {
      setPreview(entry)
      return
    }
    if (entry.hasCache) {
      setPreview(entry)
      return
    }
    void run(t('正在缓存…'), async () => {
      await call('remote_download', { path: entry.path })
      setPreview(entry)
      pane.reload()
    })
  }
  const openTrash = () => {
    const destination = 'trash://'
    setPreview(null)
    if (x.path === destination) x.reload()
    else x.navigate(destination)
  }
  const openApplications = () => {
    setPreview(null)
    if (x.path === 'applications://') x.reload()
    else x.navigate('applications://')
  }
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
    void run(t('正在打开终端…'), () =>
      call('open_terminal', { path: first?.isDir ? first.path : x.path, id }),
    )
  }
  const openIde = (path = first?.path || x.path) => {
    if (!writable()) return
    if (!path) return
    const ide = ides.find((item) => item.id === prefs.ide) || ides[0]
    if (!ide) {
      x.setError(t('没有检测到可用的 IDE'))
      return
    }
    void run(t('正在用 {0} 打开…', ide.name), () => call('open_in_ide', { path, id: ide.id }))
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
      notify(t('标签已更新'))
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
