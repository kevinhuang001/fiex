import { useExplorerShortcuts } from './useExplorerShortcuts'
import { useExplorer } from '@/hooks/useExplorer'
import { usePreferences } from '@/hooks/usePreferences'
import { call, native } from '@/lib/api'
import type { DialogState } from '@/lib/dialogs'
import { basename } from '@/lib/files'
import { shortcutLabel } from '@/lib/shortcuts'
import type { Entry, Location, Tag } from '@/lib/types'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useDesktopEnvironment } from './useDesktopEnvironment'
import { useFileOperations } from './useFileOperations'
import { usePathEditor } from './usePathEditor'
export function useAppController() {
  const [prefs, update] = usePreferences()
  const x = useExplorer(prefs)
  const { terminals, ides, platform, trashPath } = useDesktopEnvironment()

  const [sidebar, setSidebar] = useState(true)
  const [preview, setPreview] = useState<Entry | null>(null)
  const [settings, setSettings] = useState(false)
  const [info, setInfo] = useState<Entry | null>(null)
  const [dialog, setDialog] = useState<DialogState | null>(null)
  const [dialogValue, setDialogValue] = useState('')
  const [dialogError, setDialogError] = useState('')
  const [dialogBusy, setDialogBusy] = useState(false)
  const [toast, setToast] = useState('')
  const [trashIssue, setTrashIssue] = useState(false)
  const [tagOpen, setTagOpen] = useState(false)
  const [connectionOpen, setConnectionOpen] = useState(false)
  const [gitOpen, setGitOpen] = useState(false)
  const gitBase = useRef<{ root: string; branch: string } | null>(null)
  const [tagPaths, setTagPaths] = useState<string[]>([])
  const [tagInitial, setTagInitial] = useState<Tag[]>([])
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry } | null>(null)
  const [compressOpen, setCompressOpen] = useState(false)
  const [dragging, setDragging] = useState(false)
  const searchInput = useRef<HTMLInputElement>(null)
  const chosen = x.entries.filter((e) => x.selected.includes(e.path))
  const first = chosen[0]
  const notify = useCallback((s: string) => setToast(s), [])
  useEffect(() => {
    if (!x.git) {
      gitBase.current = null
      return
    }
    if (gitBase.current?.root !== x.git.root) {
      gitBase.current = { root: x.git.root, branch: x.git.branch }
    }
  }, [x.git?.root])
  const gitSwitched = Boolean(x.git && gitBase.current && x.git.branch !== gitBase.current.branch)
  useEffect(() => {
    x.setReadOnly(gitSwitched)
  }, [gitSwitched])
  const writable = () => {
    if (!gitSwitched) return true
    x.setError('当前是只读分支或版本，切换回工作区分支后才能修改')
    return false
  }
  useEffect(() => {
    if (!x.path || settings || dialog || preview || info) return
    const frame = requestAnimationFrame(() => searchInput.current?.focus())
    return () => cancelAnimationFrame(frame)
  }, [x.path])
  useEffect(() => {
    if (!toast) return
    const timer = setTimeout(() => setToast(''), 5000)
    return () => clearTimeout(timer)
  }, [toast])
  useEffect(() => {
    if (dialog) {
      setDialogValue(dialog.value || '')
      setDialogError('')
    }
  }, [dialog])

  const {
    busy,
    clipboard,
    setClipboard,
    run,
    create,
    recycle,
    transfer,
    copy,
    duplicate,
    compress,
    extract,
    undo,
  } = useFileOperations({ x, prefs, writable, notify, setDialog, chosen, first })

  const open = (e: Entry) => {
    if (e.isDir && e.extension !== 'app') {
      setPreview(null)
      x.navigate(e.path)
      return
    }
    if (gitSwitched) {
      previewEntry(e)
      return
    }
    void run('正在打开…', () => call('open_path', { path: e.path }))
  }
  const previewEntry = (entry: Entry) => {
    if (!isRemotePath(entry.path)) {
      setPreview(entry)
      return
    }
    void run('正在下载…', async () => {
      await call('remote_download', { path: entry.path })
      setPreview(entry)
      x.reload()
    })
  }
  const ejectDevice = (path: string) =>
    setDialog({
      title: '弹出设备',
      description: '确认弹出这个磁盘？',
      submit: '弹出',
      danger: true,
      action: () => call('eject_device', { path }),
    })
  const mountDevice = (device: string) => {
    void run('正在请求挂载…', async () => {
      const path = await call<string>('mount_device', { device })
      await x.refreshLocations()
      x.navigate(path)
      notify('设备已挂载')
    })
  }
  const ejectUsb = (device: string) =>
    setDialog({
      title: '弹出 U 盘',
      description: '确认弹出这个 U 盘？请先等待写入完成。',
      submit: '弹出',
      danger: true,
      action: async () => {
        const volume = x.locations.find((location) => location.usbfs && location.device === device)
        await call('eject_usb', { device })
        await x.refreshLocations()
        if (volume && x.path.startsWith(volume.path)) {
          const home = x.locations.find((location) => location.kind === 'favorite')?.path
          x.navigate(home || '/')
        }
      },
    })
  const removeConnection = (id: string) => {
    const connection = x.connections.find((item) => item.id === id)
    if (!connection) return
    setDialog({
      title: '关闭连接',
      description: `关闭“${connection.name}”？已下载的文件会保留在 ~/.fiex。`,
      submit: '关闭连接',
      danger: true,
      action: async () => {
        await call('remove_remote_connection', { id })
        await x.refreshConnections()
        if (x.path === connection.mountPath || x.path.startsWith(connection.mountPath + '/')) {
          const home = x.locations.find((location) => location.kind === 'favorite')?.path
          x.navigate(home || '/')
        }
      },
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
  const remoteConnection = (path: string) =>
    x.connections.find(
      (connection) => path === connection.mountPath || path.startsWith(connection.mountPath + '/'),
    )
  const isRemotePath = (path: string) => Boolean(remoteConnection(path))
  const displayName = (path: string) => {
    if (trashPath && path === trashPath) return '废纸篓'
    const volume = x.locations.find((location) => location.usbfs && location.path === path)
    if (volume) return volume.name
    const connection = remoteConnection(path)
    if (connection && path === connection.mountPath) return connection.name
    return basename(path)
  }

  const {
    editingPath,
    pathDraft,
    setPathDraft,
    pathInput,
    setEditingPath,
    startPathEdit,
    submitPath,
    crumbs,
  } = usePathEditor({ x, prefs, displayName })

  const hint = (action: Parameters<typeof shortcutLabel>[1]) =>
    shortcutLabel(prefs.shortcuts, action, prefs.shortcutStyle)
  const terminal = (id = prefs.terminal) => {
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
  const bookmarkPaths = async (paths: string[]) => {
    const existing = new Set(prefs.favorites.map((item) => item.path))
    const additions: Location[] = []
    let files = 0
    for (const path of paths) {
      if (existing.has(path)) continue
      try {
        const info = await call<Entry>('file_info', { path })
        if (!info.isDir) {
          files += 1
          continue
        }
        existing.add(path)
        additions.push({
          name: info.name,
          path,
          kind: 'favorite',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        })
      } catch {
        /* The item may have disappeared while dragging. */
      }
    }
    if (additions.length) {
      update({ favorites: [...prefs.favorites, ...additions] })
      notify(`已收藏 ${additions.length} 个文件夹`)
    } else if (files) {
      notify('只能把文件夹加入收藏')
    } else {
      notify('这些文件夹已在收藏中')
    }
  }
  const favorite = () => {
    const path = first?.isDir ? first.path : x.path
    if (prefs.favorites.some((l) => l.path === path)) {
      update({ favorites: prefs.favorites.filter((l) => l.path !== path) })
      notify('已移除收藏')
    } else {
      update({
        favorites: [
          ...prefs.favorites,
          {
            name: basename(path),
            path,
            kind: 'favorite',
            available: 0,
            total: 0,
            removable: false,
            filesystem: '',
            device: '',
            mounted: true,
            usbfs: false,
          },
        ],
      })
      notify('已添加到收藏')
    }
  }
  const navigatePreview = (delta: number) => {
    if (!preview) return
    const i = x.entries.findIndex((e) => e.path === preview.path)
    const next = x.entries[(i + delta + x.entries.length) % x.entries.length]
    if (next) {
      setPreview(next)
      x.setSelected([next.path])
    }
  }
  useExplorerShortcuts({
    shortcuts: prefs.shortcuts,
    blocked: Boolean(
      dialog || settings || preview || info || tagOpen || connectionOpen || trashIssue,
    ),
    handlers: {
      settings: () => setSettings(true),
      search: () => searchInput.current?.focus(),
      goToFolder: startPathEdit,
      selectAll: () => x.setSelected(x.entries.map((entry) => entry.path)),
      copy: () => copy(false),
      cut: () => copy(true),
      paste: () => {
        if (clipboard) transfer(clipboard.paths, x.path, clipboard.moving)
      },
      undo,
      newFolder: () => {
        void create(true)
      },
      newFile: () => {
        void create(false)
      },
      newTab: () => x.newTab(),
      closeTab: () => x.closeTab(x.tab),
      duplicate,
      back: x.back,
      forward: x.forward,
      up: x.up,
      quickLook: (event) => {
        if (first && !event.repeat) previewEntry(first)
      },
      rename: () => {
        if (first) x.beginRename(first.path)
      },
      open: () => {
        if (first) open(first)
      },
      trash: () => recycle(false),
      refresh: x.reload,
      toggleSidebar: () => setSidebar((value) => !value),
      info: () => {
        if (first) setInfo(first)
      },
    },
    onEscape: () => {
      setMenu(null)
      x.setSelected([])
      x.setQuery('')
    },
  })
  useEffect(() => {
    if (!native) return
    let active = true
    let dispose: (() => void) | undefined
    void getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === 'enter') {
          setDragging(true)
        } else if (event.payload.type === 'leave') {
          setDragging(false)
        } else if (event.payload.type === 'drop') {
          setDragging(false)
          transfer(event.payload.paths, x.path, false)
        }
      })
      .then((fn) => {
        if (active) dispose = fn
        else fn()
      })
    return () => {
      active = false
      dispose?.()
    }
  }, [x.path])
  useEffect(() => {
    if (!menu) {
      setCompressOpen(false)
      return
    }
    const close = () => setMenu(null)
    document.addEventListener('click', close)
    document.addEventListener('keydown', close)
    return () => {
      document.removeEventListener('click', close)
      document.removeEventListener('keydown', close)
    }
  }, [menu])
  const control = (action: 'close' | 'minimize' | 'toggleMaximize') => {
    if (native)
      void getCurrentWindow()
        [action]()
        .catch((e) => x.setError(String(e)))
  }
  return {
    sidebar,
    dragging,
    prefs,
    x,
    update,
    favorite,
    control,
    openTrash,
    openApplications,
    setConnectionOpen,
    removeConnection,
    ejectDevice,
    ejectUsb,
    mountDevice,
    trashPath,
    setSidebar,
    displayName,
    gitSwitched,
    hint,
    create,
    gitOpen,
    setGitOpen,
    ides,
    openIde,
    terminal,
    setSettings,
    startPathEdit,
    editingPath,
    submitPath,
    pathInput,
    pathDraft,
    setPathDraft,
    setEditingPath,
    searchInput,
    crumbs,
    open,
    previewEntry,
    setMenu,
    transfer,
    bookmarkPaths,
    busy,
    chosen,
    clipboard,
    setClipboard,
    menu,
    setInfo,
    platform,
    openTags,
    isRemotePath,
    run,
    copy,
    duplicate,
    compress,
    extract,
    notify,
    recycle,
    setCompressOpen,
    compressOpen,
    preview,
    setPreview,
    navigatePreview,
    dialog,
    dialogBusy,
    setDialog,
    setDialogBusy,
    setDialogError,
    dialogValue,
    setDialogValue,
    dialogError,
    trashIssue,
    setTrashIssue,
    info,
    tagOpen,
    tagInitial,
    setTagOpen,
    applyTags,
    connectionOpen,
    settings,
    terminals,
    toast,
    setToast,
  }
}
export type AppController = ReturnType<typeof useAppController>
