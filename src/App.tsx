import { Sidebar } from './components/Sidebar'
import { PreferencesDialog } from './components/PreferencesDialog'
import { FileInfoDialog } from './components/FileInfoDialog'
import { TagDialog } from './components/TagDialog'
import { ConnectionDialog } from './components/ConnectionDialog'
import { GitPanel } from './components/GitPanel'
import { useCallback, useEffect, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import {
  ArrowDownUp,
  ArrowUp,
  ChevronDown,
  Code2,
  ChevronLeft,
  ChevronRight,
  Columns3,
  Copy,
  Download,
  Eye,
  Folder,
  FolderPlus,
  FolderTree,
  GitBranch,
  Settings2,
  Grid2X2,
  HardDrive,
  Image,
  Info,
  List,
  LoaderCircle,
  PanelLeft,
  Plus,
  RefreshCw,
  Search,
  TerminalSquare,
  Trash2,
  X,
  CheckCircle2,
  Archive,
  ExternalLink,
  Scissors,
  Pencil,
  Tags,
} from 'lucide-react'
import type { CSSProperties, ReactNode } from 'react'
import { FileViews } from './components/browser/FileViews'
import { Preview } from './components/Preview'
import { Button } from './components/ui/button'
import { Modal } from './components/ui/Modal'
import { Menu, Choice, Label } from './components/Menus'
import { useExplorer } from './hooks/useExplorer'
import { usePreferences } from './hooks/usePreferences'
import { call, native, demo } from './lib/api'
import { basename, isArchive, join, parent, size, uniqueName } from './lib/files'
import { matchShortcut, shortcutLabel } from './lib/shortcuts'
import type {
  Entry,
  Group,
  Location,
  OperationResult,
  Sort,
  Tag,
  Terminal,
  View,
} from './lib/types'
const views: { id: View; name: string; icon: ReactNode }[] = [
  { id: 'icons', name: '图标', icon: <Grid2X2 /> },
  { id: 'list', name: '列表', icon: <List /> },
  { id: 'columns', name: '分栏', icon: <Columns3 /> },
  { id: 'gallery', name: '画廊', icon: <Image /> },
]
const groupOptions: [Group, string][] = [
  ['none', '无'],
  ['kind', '种类'],
  ['modified', '修改时间'],
  ['extension', '文件扩展名'],
  ['size', '文件大小'],
]
const sortOptions: [Sort, string][] = [
  ['name', '名称'],
  ['modified', '修改时间'],
  ['size', '大小'],
  ['kind', '种类'],
]
const archiveFormats = [
  { id: 'zip', name: 'ZIP', ext: '.zip' },
  { id: '7z', name: '7Z', ext: '.7z' },
  { id: 'tar.gz', name: 'TAR.GZ', ext: '.tar.gz' },
  { id: 'tar', name: 'TAR', ext: '.tar' },
  { id: 'tar.bz2', name: 'TAR.BZ2', ext: '.tar.bz2' },
  { id: 'tar.xz', name: 'TAR.XZ', ext: '.tar.xz' },
] as const
type ArchiveFormat = (typeof archiveFormats)[number]
interface ContextAction {
  label: string
  icon: ReactNode
  hint: string
  action: () => void
}
interface ContextSubmenu {
  label: string
  icon: ReactNode
  hint: string
  submenu: { label: string; action: () => void }[]
}
type ContextItem = ContextAction | ContextSubmenu
interface DialogState {
  title: string
  label?: string
  value?: string
  description?: string
  submit: string
  action: (value: string) => Promise<unknown>
  danger?: boolean
}
export default function App() {
  const [prefs, update] = usePreferences()
  const x = useExplorer(prefs)
  const [sidebar, setSidebar] = useState(true)
  const [preview, setPreview] = useState<Entry | null>(null)
  const [settings, setSettings] = useState(false)
  const [info, setInfo] = useState<Entry | null>(null)
  const [dialog, setDialog] = useState<DialogState | null>(null)
  const [dialogValue, setDialogValue] = useState('')
  const [dialogError, setDialogError] = useState('')
  const [dialogBusy, setDialogBusy] = useState(false)
  const [busy, setBusy] = useState('')
  const [toast, setToast] = useState('')
  const [terminals, setTerminals] = useState<Terminal[]>([])
  const [platform, setPlatform] = useState('')
  const [trashPath, setTrashPath] = useState<string | null>(null)
  const [trashIssue, setTrashIssue] = useState(false)
  const [tagOpen, setTagOpen] = useState(false)
  const [connectionOpen, setConnectionOpen] = useState(false)
  const [gitOpen, setGitOpen] = useState(false)
  const [tagPaths, setTagPaths] = useState<string[]>([])
  const [tagInitial, setTagInitial] = useState<Tag[]>([])
  const [clipboard, setClipboard] = useState<{ paths: string[]; moving: boolean } | null>(null)
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry } | null>(null)
  const [compressOpen, setCompressOpen] = useState(false)
  const [editingPath, setEditingPath] = useState(false)
  const [pathDraft, setPathDraft] = useState('')
  const [dragging, setDragging] = useState(false)
  const searchInput = useRef<HTMLInputElement>(null)
  const pathInput = useRef<HTMLInputElement>(null)
  const chosen = x.entries.filter((e) => x.selected.includes(e.path))
  const first = chosen[0]
  const notify = useCallback((s: string) => setToast(s), [])
  useEffect(() => {
    void call<string>('platform')
      .then(setPlatform)
      .catch(() => {})
    void call<Terminal[]>('detect_terminals')
      .then(setTerminals)
      .catch(() => {})
    void call<string | null>('trash_path')
      .then(setTrashPath)
      .catch(() => {})
  }, [])
  // Opening a folder puts the cursor in search so typing immediately scopes
  // the new location, while Escape clears the query and leaves the field.
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
  const run = async <T,>(label: string, fn: () => Promise<T>): Promise<T | undefined> => {
    if (busy) return
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
      setBusy('')
    }
  }
  const handleResult = (result: OperationResult) => {
    if (result.errors.length)
      throw new Error(`${result.paths.length} 项已完成。${result.errors.join('\n')}`)
    notify(
      result.skipped
        ? `${result.paths.length} 项操作完成，已按忽略规则跳过 ${result.skipped} 项`
        : `${result.paths.length} 项操作完成`,
    )
  }
  const open = (e: Entry) => {
    if (e.isDir && e.extension !== 'app') {
      setPreview(null)
      x.navigate(e.path)
    } else void run('正在打开…', () => call('open_path', { path: e.path }))
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
  const create = async (folder: boolean) => {
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
    const connection = remoteConnection(path)
    if (connection && path === connection.mountPath) return connection.name
    return basename(path)
  }
  const startPathEdit = () => {
    setPathDraft(x.path)
    setEditingPath(true)
  }
  useEffect(() => {
    if (!editingPath) return
    const input = pathInput.current
    input?.focus()
    input?.select()
  }, [editingPath])
  const submitPath = async (event: React.FormEvent) => {
    event.preventDefault()
    const target = pathDraft.trim()
    if (!target) return
    try {
      const { path } = await call<{ path: string }>('list_directory', {
        path: target,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
      })
      setEditingPath(false)
      x.navigate(path)
    } catch (error) {
      x.setError(String(error))
    }
  }
  const hint = (action: Parameters<typeof shortcutLabel>[1]) =>
    shortcutLabel(prefs.shortcuts, action, prefs.shortcutStyle)
  const recycle = () => {
    if (!x.selected.length) return
    const paths = [...x.selected]
    setDialog({
      title: `将 ${paths.length} 个项目移到废纸篓？`,
      description: '项目将移入系统废纸篓，可在系统文件管理器中恢复。',
      submit: '移到废纸篓',
      danger: true,
      action: async () => {
        handleResult(await call<OperationResult>('recycle', { paths }))
        x.setSelected([])
        x.reload()
      },
    })
  }
  const transfer = (paths: string[], destination: string, moving: boolean) => {
    if (!paths.length) return
    setDialog({
      title: moving ? '移动项目' : '复制项目',
      description: `${paths.length} 个项目 → ${destination}\n遇到同名项目时自动保留两者，不替换已有文件。`,
      submit: moving ? '移动（保留两者）' : '复制（保留两者）',
      action: async () => {
        handleResult(
          await call<OperationResult>('transfer', {
            paths,
            destination,
            moving,
            keepBoth: true,
            ignores: prefs.copyIgnore,
          }),
        )
        if (moving) setClipboard(null)
        x.reload()
      },
    })
  }
  const copy = (moving: boolean) => {
    if (x.selected.length) {
      setClipboard({ paths: [...x.selected], moving })
      notify(moving ? '已剪切，选择目标文件夹后粘贴' : '已复制，选择目标文件夹后粘贴')
    }
  }
  const duplicate = () =>
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
  const compress = (format: ArchiveFormat) => {
    if (!first) return
    setDialog({
      title: `压缩为 ${format.name}`,
      label: '压缩包名称',
      value: (chosen.length === 1 ? first.name : 'Archive') + format.ext,
      submit: '压缩',
      description: '已按设置中的压缩忽略规则过滤。',
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
    if (first)
      void run('正在解压…', async () => {
        const path = await call<string>('extract_archive', { path: first.path })
        notify(`已解压到 ${basename(path)}`)
      })
  }
  const terminal = (id = prefs.terminal) =>
    void run('正在打开终端…', () =>
      call('open_terminal', { path: first?.isDir ? first.path : x.path, id }),
    )
  const openVSCode = (path = first?.path || x.path) => {
    if (!path) return
    void run('正在用 VS Code 打开…', () => call('open_in_vscode', { path }))
  }
  const undo = () =>
    void run('正在撤销…', async () => {
      await call('undo')
      notify('已撤销上一次文件操作')
    })
  const openTags = (paths: string[]) => {
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
        additions.push({ name: info.name, path, kind: 'favorite', available: 0, total: 0 })
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
          { name: basename(path), path, kind: 'favorite', available: 0, total: 0 },
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
  useEffect(() => {
    const fn = (event: KeyboardEvent) => {
      const target = event.target instanceof Element ? event.target : null
      if (
        event.defaultPrevented ||
        event.isComposing ||
        target?.closest('input,textarea,select,[contenteditable=true],[role=dialog],[role=menu]') ||
        dialog ||
        settings ||
        preview ||
        info
      )
        return
      // Let a focused control handle Space/Return itself; modifier shortcuts
      // still work while a toolbar button has focus.
      if (
        (event.key === 'Enter' || event.key === ' ') &&
        target?.closest('button,a,select,[role=button],[role=tab]')
      )
        return
      const action = matchShortcut(event, prefs.shortcuts)
      if (!action) {
        if (event.key === 'Escape') {
          setMenu(null)
          x.setSelected([])
          x.setQuery('')
        }
        return
      }
      event.preventDefault()
      switch (action) {
        case 'settings':
          setSettings(true)
          break
        case 'search':
          searchInput.current?.focus()
          break
        case 'goToFolder':
          startPathEdit()
          break
        case 'selectAll':
          x.setSelected(x.entries.map((e) => e.path))
          break
        case 'copy':
          copy(false)
          break
        case 'cut':
          copy(true)
          break
        case 'paste':
          if (clipboard) transfer(clipboard.paths, x.path, clipboard.moving)
          break
        case 'undo':
          undo()
          break
        case 'newFolder':
          void create(true)
          break
        case 'newFile':
          void create(false)
          break
        case 'newTab':
          x.newTab()
          break
        case 'closeTab':
          x.closeTab(x.tab)
          break
        case 'duplicate':
          duplicate()
          break
        case 'back':
          x.back()
          break
        case 'forward':
          x.forward()
          break
        case 'up':
          x.up()
          break
        case 'quickLook':
          if (first && !event.repeat) previewEntry(first)
          break
        case 'rename':
          if (first) x.beginRename(first.path)
          break
        case 'open':
          if (first) open(first)
          break
        case 'trash':
          recycle()
          break
        case 'refresh':
          x.reload()
          break
        case 'toggleSidebar':
          setSidebar((value) => !value)
          break
        case 'info':
          if (first) setInfo(first)
          break
      }
    }
    document.addEventListener('keydown', fn)
    return () => document.removeEventListener('keydown', fn)
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
  const pathParts = x.path.split(/[\\/]/).filter(Boolean)
  let accumulated = x.path.startsWith('/') ? '' : x.path.startsWith('\\') ? '\\\\' : ''
  const crumbs = pathParts.map((name) => {
    accumulated = accumulated
      ? join(accumulated, name)
      : x.path.startsWith('/')
        ? '/' + name
        : name + '\\'
    return { name: displayName(accumulated) || name, path: accumulated }
  })
  return (
    <div
      className={`app ${sidebar ? '' : 'sidebar-hidden'} ${dragging ? 'dragging' : ''}`}
      style={{ '--label-size': `${prefs.fontSize}px` } as CSSProperties}
    >
      <Sidebar
        x={x}
        prefs={prefs}
        update={update}
        favorite={favorite}
        control={control}
        onTrash={openTrash}
        onApplications={openApplications}
        onAddConnection={() => setConnectionOpen(true)}
        onRemoveConnection={removeConnection}
        onEject={ejectDevice}
        trashPath={trashPath}
      />
      <main className="main">
        <header className="toolbar" data-tauri-drag-region>
          {!sidebar && (
            <div className="traffic">
              <button title="关闭窗口" className="traffic-close" onClick={() => control('close')} />
              <button
                title="最小化"
                className="traffic-minimize"
                onClick={() => control('minimize')}
              />
              <button
                title="最大化"
                className="traffic-maximize"
                onClick={() => control('toggleMaximize')}
              />
            </div>
          )}
          <Button
            variant="ghost"
            size="icon"
            aria-label="切换侧栏"
            title="切换侧栏"
            onClick={() => setSidebar(!sidebar)}
          >
            <PanelLeft />
          </Button>
          <div className="navigation-controls">
            <Button
              variant="ghost"
              size="icon"
              disabled={!x.canBack}
              title="后退"
              aria-label="后退"
              onClick={x.back}
            >
              <ChevronLeft />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              disabled={!x.canForward}
              title="前进"
              aria-label="前进"
              onClick={x.forward}
            >
              <ChevronRight />
            </Button>
          </div>
          <h1 data-tauri-drag-region>{x.query ? '搜索' : displayName(x.path) || 'Fiex'}</h1>
          <div className="toolbar-spacer" data-tauri-drag-region />
          <div className="view-switch">
            {views.map((v) => (
              <Button
                key={v.id}
                variant="ghost"
                size="icon"
                title={v.name}
                aria-label={`${v.name}视图`}
                aria-pressed={prefs.view === v.id}
                onClick={() => update({ view: v.id })}
              >
                {v.icon}
              </Button>
            ))}
          </div>
          <Menu label="分组方式" icon={<FolderTree />}>
            <Label>分组方式</Label>
            {groupOptions.map(([id, name]) => (
              <Choice key={id} checked={prefs.group === id} onClick={() => update({ group: id })}>
                {name}
              </Choice>
            ))}
          </Menu>
          <Menu label="排序方式" icon={<ArrowDownUp />}>
            <Label>排序方式</Label>
            {sortOptions.map(([id, name]) => (
              <Choice key={id} checked={prefs.sort === id} onClick={() => update({ sort: id })}>
                {name}
              </Choice>
            ))}
            <Choice
              checked={prefs.descending}
              onClick={() => update({ descending: !prefs.descending })}
            >
              降序
            </Choice>
          </Menu>
          <Button
            variant="ghost"
            size="icon"
            title={`新建文件夹 ${hint('newFolder')}`}
            aria-label="新建文件夹"
            onClick={() => create(true)}
          >
            <FolderPlus />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className={`git-toolbar-button ${gitOpen ? 'active' : ''}`}
            title="源代码管理"
            aria-label="源代码管理"
            aria-pressed={gitOpen}
            onClick={() => setGitOpen((value) => !value)}
          >
            <GitBranch />
            {x.git && Object.keys(x.git.entries).length > 0 && (
              <span className="git-toolbar-count">{Object.keys(x.git.entries).length}</span>
            )}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title="在 VS Code 中打开"
            aria-label="在 VS Code 中打开"
            onClick={() => openVSCode()}
          >
            <Code2 />
          </Button>
          <div className="toolbar-divider" />
          <div className="terminal-control">
            <Button
              variant="ghost"
              size="icon"
              title="Open in Terminal"
              aria-label="Open in Terminal"
              onClick={() => terminal()}
            >
              <TerminalSquare />
            </Button>
            <Menu label="选择终端" icon={<ChevronDown />}>
              {terminals.map((t) => (
                <Choice
                  key={t.id}
                  checked={prefs.terminal === t.id}
                  onClick={() => {
                    update({ terminal: t.id })
                    terminal(t.id)
                  }}
                >
                  {t.name}
                </Choice>
              ))}
            </Menu>
          </div>
          <Button
            variant="ghost"
            size="icon"
            title="设置"
            aria-label="设置"
            onClick={() => setSettings(true)}
          >
            <Settings2 />
          </Button>
        </header>
        <div className="tabs" role="tablist" aria-label="标签页">
          {x.tabs.map((path, i) => (
            <div className={`tab ${i === x.tab ? 'active' : ''}`} key={i}>
              <button role="tab" aria-selected={i === x.tab} onClick={() => x.switchTab(i)}>
                <Folder size={13} />
                <span>{displayName(path)}</span>
              </button>
              {x.tabs.length > 1 && (
                <button aria-label="关闭标签页" onClick={() => x.closeTab(i)}>
                  <X size={12} />
                </button>
              )}
            </div>
          ))}
          <button
            className="icon-button"
            title="新建标签页"
            aria-label="新建标签页"
            onClick={() => x.newTab()}
          >
            <Plus size={15} />
          </button>
          <div className="tabs-spacer" data-tauri-drag-region />
        </div>
        <div className="path-toolbar">
          <Button variant="ghost" size="icon" title="上一级" aria-label="上一级" onClick={x.up}>
            <ArrowUp />
          </Button>
          <nav
            className="breadcrumbs"
            aria-label="当前位置"
            onClick={(event) => {
              if (event.target === event.currentTarget) startPathEdit()
            }}
          >
            <button onClick={startPathEdit} title="编辑或复制路径" aria-label="编辑路径">
              <HardDrive size={13} />
            </button>
            {editingPath ? (
              <form className="path-editor" onSubmit={submitPath}>
                <input
                  ref={pathInput}
                  className="path-input"
                  aria-label="文件夹路径"
                  value={pathDraft}
                  spellCheck={false}
                  onChange={(event) => setPathDraft(event.target.value)}
                  onBlur={() => setEditingPath(false)}
                  onKeyDown={(event) => {
                    event.stopPropagation()
                    if (event.key === 'Escape') {
                      event.preventDefault()
                      setEditingPath(false)
                      searchInput.current?.focus()
                    }
                  }}
                />
              </form>
            ) : (
              <>
                {crumbs.map((p, i) => (
                  <span key={i}>
                    <ChevronRight size={11} />
                    <button
                      onClick={() =>
                        i === crumbs.length - 1 ? startPathEdit() : x.navigate(p.path)
                      }
                    >
                      {p.name}
                    </button>
                  </span>
                ))}
              </>
            )}
          </nav>
          <div className="search-box">
            <Search size={15} />
            <input
              ref={searchInput}
              aria-label="搜索文件"
              placeholder="搜索此文件夹"
              value={x.query}
              onChange={(e) => x.setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key !== 'Escape') return
                e.stopPropagation()
                x.setQuery('')
                e.currentTarget.blur()
              }}
            />
            {x.searching ? (
              <LoaderCircle className="spin" size={14} />
            ) : x.query ? (
              <button className="icon-button" aria-label="清除搜索" onClick={() => x.setQuery('')}>
                <X size={13} />
              </button>
            ) : (
              <kbd>{hint('search')}</kbd>
            )}
          </div>
        </div>
        {demo && <div className="demo-note">界面演示 · 桌面应用连接真实文件系统</div>}
        {x.error && (
          <div className="error-banner" role="alert">
            <Info size={16} />
            <span>{x.error}</span>
            <button className="icon-button" title="关闭提示" onClick={() => x.setError('')}>
              <X size={15} />
            </button>
          </div>
        )}
        {x.warnings.length > 0 && (
          <details className="warnings">
            <summary>{x.warnings.length} 个项目无法读取</summary>
            {x.warnings.join('\n')}
          </details>
        )}
        {x.loading ? (
          <div className="loading">
            <LoaderCircle className="spin" />
            正在读取文件夹…
          </div>
        ) : (
          <FileViews
            explorer={x}
            prefs={prefs}
            onOpen={open}
            onPreview={previewEntry}
            onMenu={(entry, mx, my) => setMenu({ entry, x: mx, y: my })}
            onDrop={transfer}
            onBookmark={bookmarkPaths}
          />
        )}
        <GitPanel
          open={gitOpen}
          git={x.git}
          onClose={() => setGitOpen(false)}
          onRefresh={x.reload}
          onOpen={(path) => x.navigate(parent(path))}
        />
        <footer className="statusbar">
          <span>
            {busy ? (
              <>
                <LoaderCircle className="spin" size={13} />
                {busy}
              </>
            ) : x.searching ? (
              <>
                <LoaderCircle className="spin" size={13} />
                正在搜索…
              </>
            ) : (
              <>
                {x.entries.length.toLocaleString()} 个项目
                {x.selected.length > 0 && ` · 已选择 ${x.selected.length} 项`}
                {chosen.length > 0 &&
                  !chosen.some((e) => e.isDir) &&
                  ` · ${size(chosen.reduce((a, e) => a + e.size, 0))}`}
              </>
            )}
          </span>
          {x.git && (
            <span className="git-status" title={x.git.root}>
              <GitBranch size={12} />
              {x.git.branch}
              {Object.keys(x.git.entries).length > 0 &&
                ` · ${Object.keys(x.git.entries).length} 更改`}
            </span>
          )}
          {x.tagFilter && <span>标签：{x.tagFilter}</span>}
          <span>{x.searchStats}</span>
          <div className="toolbar-spacer" />
          {clipboard && (
            <button title="清除剪贴板" onClick={() => setClipboard(null)}>
              {clipboard.moving ? '剪切' : '复制'} {clipboard.paths.length} 项 <X size={11} />
            </button>
          )}
          <button title="刷新" aria-label="刷新" onClick={x.reload}>
            <RefreshCw size={13} />
          </button>
        </footer>
      </main>
      {dragging && (
        <div className="drop-overlay">
          <Download />
          <strong>复制到 {displayName(x.path)}</strong>
        </div>
      )}
      {menu && (
        <div
          role="menu"
          aria-label="文件操作"
          className="context-menu"
          style={{
            left: Math.min(menu.x, innerWidth - 360),
            top: Math.max(8, Math.min(menu.y, innerHeight - 380)),
          }}
        >
          {(
            [
              {
                label: '打开',
                icon: <ExternalLink />,
                hint: hint('open'),
                action: () => open(menu.entry),
              },
              {
                label: '快速查看',
                icon: <Eye />,
                hint: hint('quickLook'),
                action: () => previewEntry(menu.entry),
              },
              {
                label: '显示简介',
                icon: <Info />,
                hint: hint('info'),
                action: () => setInfo(menu.entry),
              },
              ...(platform === 'macos'
                ? [
                    {
                      label: '标签…',
                      icon: <Tags />,
                      hint: '',
                      action: () =>
                        openTags(
                          x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path],
                        ),
                    },
                  ]
                : []),
              {
                label: '重新命名',
                icon: <Pencil />,
                hint: hint('rename'),
                action: () => x.beginRename(menu.entry.path),
              },
              { label: '复制', icon: <Copy />, hint: hint('copy'), action: () => copy(false) },
              { label: '剪切', icon: <Scissors />, hint: hint('cut'), action: () => copy(true) },
              { label: '创建副本', icon: <Copy />, hint: hint('duplicate'), action: duplicate },
              {
                label: '压缩为…',
                icon: <Archive />,
                hint: '',
                submenu: archiveFormats.map((format) => ({
                  label: format.name,
                  action: () => compress(format),
                })),
              },
              ...(isArchive(menu.entry)
                ? [{ label: '解压缩', icon: <Archive />, hint: '', action: extract }]
                : []),
              {
                label: 'Open in Terminal',
                icon: <TerminalSquare />,
                hint: '',
                action: () => terminal(),
              },
              {
                label: '复制路径',
                icon: <Copy />,
                hint: '',
                action: () =>
                  void navigator.clipboard
                    .writeText(menu.entry.path)
                    .then(() => notify('路径已复制'))
                    .catch((e) => x.setError(String(e))),
              },
              { label: '移到废纸篓', icon: <Trash2 />, hint: hint('trash'), action: recycle },
            ] as ContextItem[]
          ).map((a) =>
            'submenu' in a ? (
              <div
                className="context-submenu-wrap"
                key={a.label}
                onMouseEnter={() => setCompressOpen(true)}
                onMouseLeave={() => setCompressOpen(false)}
              >
                <button
                  role="menuitem"
                  aria-haspopup="menu"
                  aria-expanded={compressOpen}
                  onClick={(event) => {
                    event.stopPropagation()
                    setCompressOpen((value) => !value)
                  }}
                >
                  {a.icon}
                  {a.label}
                  <ChevronRight className="submenu-arrow" size={13} />
                </button>
                {compressOpen && (
                  <div className="context-submenu" role="menu">
                    {a.submenu.map((item) => (
                      <button
                        role="menuitem"
                        key={item.label}
                        onClick={() => {
                          item.action()
                          setMenu(null)
                        }}
                      >
                        {item.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ) : (
              <button
                role="menuitem"
                key={a.label}
                onClick={() => {
                  a.action()
                  setMenu(null)
                }}
              >
                {a.icon}
                {a.label}
                {a.hint && <span className="shortcut">{a.hint}</span>}
              </button>
            ),
          )}
        </div>
      )}
      <Preview
        entry={preview}
        onClose={() => setPreview(null)}
        onNavigate={navigatePreview}
        onOpen={open}
      />
      <Modal
        open={!!dialog}
        onClose={() => {
          if (!dialogBusy) setDialog(null)
        }}
        title={dialog?.title || ''}
        description={dialog?.description}
      >
        <form
          onSubmit={async (e) => {
            e.preventDefault()
            if (!dialog || dialogBusy) return
            setDialogBusy(true)
            setDialogError('')
            try {
              await dialog.action(dialogValue)
              setDialog(null)
            } catch (err) {
              setDialogError(String(err))
              x.reload()
            } finally {
              setDialogBusy(false)
            }
          }}
        >
          {dialog?.label && (
            <label className="field-label">
              {dialog.label}
              <input
                className="text-field"
                autoFocus
                value={dialogValue}
                onFocus={(e) => e.target.select()}
                onChange={(e) => setDialogValue(e.target.value)}
              />
            </label>
          )}
          {dialogError && (
            <p className="error" role="alert">
              {dialogError}
            </p>
          )}
          <div className="dialog-actions">
            <Button
              type="button"
              variant="ghost"
              disabled={dialogBusy}
              onClick={() => setDialog(null)}
            >
              取消
            </Button>
            <Button
              type="submit"
              variant={dialog?.danger ? 'destructive' : 'default'}
              disabled={dialogBusy || (!!dialog?.label && !dialogValue.trim())}
            >
              {dialogBusy && <LoaderCircle className="spin" />}
              {dialogBusy ? '正在处理…' : dialog?.submit}
            </Button>
          </div>
        </form>
      </Modal>
      <Modal
        open={trashIssue}
        onClose={() => setTrashIssue(false)}
        title="无法浏览废纸篓"
        description="macOS 不允许应用直接读取 ~/.Trash。请在“系统设置 → 隐私与安全性 → 完全磁盘访问权限”中允许 Fiex，随后即可在应用内浏览废纸篓；也可以现在用 Finder 打开。"
      >
        <div className="dialog-actions">
          <Button variant="ghost" onClick={() => setTrashIssue(false)}>
            取消
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              setTrashIssue(false)
              void call('open_trash').catch((error) => x.setError(String(error)))
            }}
          >
            在 Finder 中打开
          </Button>
          <Button
            onClick={() => {
              setTrashIssue(false)
              void call('open_full_disk_access').catch((error) => x.setError(String(error)))
            }}
          >
            打开权限设置
          </Button>
        </div>
      </Modal>
      <FileInfoDialog
        info={info}
        gitStatus={info ? x.git?.entries[info.path] : undefined}
        onClose={() => setInfo(null)}
        onError={x.setError}
      />
      <TagDialog
        open={tagOpen}
        initial={tagInitial}
        onClose={() => setTagOpen(false)}
        onApply={applyTags}
      />
      <ConnectionDialog
        open={connectionOpen}
        onClose={() => setConnectionOpen(false)}
        onSaved={(connection) => {
          setConnectionOpen(false)
          void x.refreshConnections()
          x.navigate(connection.mountPath)
          notify(`已连接 ${connection.name}`)
        }}
      />
      <PreferencesDialog
        open={settings}
        onClose={() => setSettings(false)}
        prefs={prefs}
        update={update}
        terminals={terminals}
        onError={x.setError}
      />
      {toast && (
        <div className="toast" role="status">
          <CheckCircle2 size={16} />
          {toast}
          <button title="关闭通知" onClick={() => setToast('')}>
            <X size={13} />
          </button>
        </div>
      )}
    </div>
  )
}
