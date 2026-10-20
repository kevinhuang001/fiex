import { Sidebar } from './components/Sidebar'
import { PreferencesDialog } from './components/PreferencesDialog'
import { FileInfoDialog } from './components/FileInfoDialog'
import { useCallback, useEffect, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import {
  ArrowDownUp,
  ArrowUp,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Columns3,
  Copy,
  Download,
  Eye,
  FilePlus2,
  Folder,
  FolderPlus,
  Grid2X2,
  HardDrive,
  Image,
  Info,
  List,
  LoaderCircle,
  MoreHorizontal,
  PanelLeft,
  Plus,
  RefreshCw,
  Search,
  Star,
  TerminalSquare,
  Trash2,
  X,
  CheckCircle2,
  Archive,
  ExternalLink,
  ClipboardPaste,
  Scissors,
  Undo2,
  Pencil,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { FileViews } from './components/browser/FileViews'
import { Preview } from './components/Preview'
import { Button } from './components/ui/button'
import { Modal } from './components/ui/Modal'
import { Menu, Choice, Action, Separator, Label } from './components/Menus'
import { useExplorer } from './hooks/useExplorer'
import { usePreferences } from './hooks/usePreferences'
import { call, native, demo } from './lib/api'
import { basename, isArchive, join, size, uniqueName } from './lib/files'
import { matchShortcut, shortcutLabel } from './lib/shortcuts'
import type { Entry, Group, OperationResult, Sort, Terminal, View } from './lib/types'
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
  const [trashPath, setTrashPath] = useState<string | null>(null)
  const [clipboard, setClipboard] = useState<{ paths: string[]; moving: boolean } | null>(null)
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry } | null>(null)
  const [editingPath, setEditingPath] = useState(false)
  const [pathDraft, setPathDraft] = useState('')
  const [dragging, setDragging] = useState(false)
  const searchInput = useRef<HTMLInputElement>(null)
  const pathInput = useRef<HTMLInputElement>(null)
  const chosen = x.entries.filter((e) => x.selected.includes(e.path))
  const first = chosen[0]
  const notify = useCallback((s: string) => setToast(s), [])
  useEffect(() => {
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
    notify(`${result.paths.length} 项操作完成`)
  }
  const open = (e: Entry) => {
    if (e.isDir && e.extension !== 'app') {
      setPreview(null)
      x.navigate(e.path)
    } else void run('正在打开…', () => call('open_path', { path: e.path }))
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
  const rename = () => {
    if (first) x.beginRename(first.path)
  }
  const openTrash = () => {
    if (trashPath) {
      setPreview(null)
      x.navigate(trashPath)
    } else {
      void run('正在打开废纸篓…', () => call('open_trash'))
    }
  }
  const openApplications = () => void run('正在打开应用程序…', () => call('open_applications'))
  const displayName = (path: string) =>
    trashPath && path === trashPath ? '废纸篓' : basename(path)
  const copyPath = (value = editingPath ? pathDraft : x.path) => {
    if (!value) return
    void navigator.clipboard
      .writeText(value)
      .then(() => notify('路径已复制'))
      .catch((error) => x.setError(String(error)))
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
          await call<OperationResult>('transfer', { paths, destination, moving, keepBoth: true }),
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
        }),
      )
    })
  const compress = () => {
    if (!first) return
    setDialog({
      title: '压缩为 ZIP',
      label: '压缩包名称',
      value: chosen.length === 1 ? first.name + '.zip' : 'Archive.zip',
      submit: '压缩',
      description: '内置 ZIP 压缩，支持文件和文件夹。',
      action: async (name) => {
        if (!name.toLowerCase().endsWith('.zip') || /[\\/]/.test(name) || name === '..')
          throw new Error('请输入以 .zip 结尾的文件名')
        await call('compress_archive', { paths: x.selected, destination: join(x.path, name) })
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
  const undo = () =>
    void run('正在撤销…', async () => {
      await call('undo')
      notify('已撤销上一次文件操作')
    })
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
          if (first && !event.repeat) setPreview(first)
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
    if (!menu) return
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
    <div className={`app ${sidebar ? '' : 'sidebar-hidden'} ${dragging ? 'dragging' : ''}`}>
      <Sidebar
        x={x}
        prefs={prefs}
        update={update}
        favorite={favorite}
        transfer={transfer}
        control={control}
        onSettings={() => setSettings(true)}
        onTrash={openTrash}
        onApplications={openApplications}
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
          <Menu label="分组和排序" icon={<ArrowDownUp />}>
            <Label>分组方式</Label>
            {groupOptions.map(([id, name]) => (
              <Choice key={id} checked={prefs.group === id} onClick={() => update({ group: id })}>
                {name}
              </Choice>
            ))}
            <Separator />
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
          <Menu label="更多操作" icon={<MoreHorizontal />}>
            <Action onClick={() => create(true)}>
              <FolderPlus />
              新建文件夹<span className="shortcut">{hint('newFolder')}</span>
            </Action>
            <Action onClick={() => create(false)}>
              <FilePlus2 />
              新建文件<span className="shortcut">{hint('newFile')}</span>
            </Action>
            <Separator />
            <Action disabled={!first} onClick={rename}>
              <Pencil />
              重新命名<span className="shortcut">{hint('rename')}</span>
            </Action>
            <Action disabled={!first} onClick={() => copy(false)}>
              <Copy />
              复制<span className="shortcut">{hint('copy')}</span>
            </Action>
            <Action disabled={!first} onClick={() => copy(true)}>
              <Scissors />
              剪切<span className="shortcut">{hint('cut')}</span>
            </Action>
            <Action
              disabled={!clipboard}
              onClick={() => clipboard && transfer(clipboard.paths, x.path, clipboard.moving)}
            >
              <ClipboardPaste />
              粘贴<span className="shortcut">{hint('paste')}</span>
            </Action>
            <Action disabled={!first} onClick={duplicate}>
              <Copy />
              创建副本<span className="shortcut">{hint('duplicate')}</span>
            </Action>
            <Action onClick={undo}>
              <Undo2 />
              撤销<span className="shortcut">{hint('undo')}</span>
            </Action>
            <Separator />
            <Action disabled={!first} onClick={compress}>
              <Archive />
              压缩为 ZIP
            </Action>
            <Action disabled={!first || !isArchive(first)} onClick={extract}>
              <Archive />
              解压缩
            </Action>
            <Action onClick={favorite}>
              <Star />
              收藏文件夹
            </Action>
            <Action disabled={!first} onClick={() => first && setInfo(first)}>
              <Info />
              显示简介<span className="shortcut">{hint('info')}</span>
            </Action>
            <Action disabled={!first} onClick={recycle}>
              <Trash2 />
              移到废纸篓<span className="shortcut">{hint('trash')}</span>
            </Action>
            <Separator />
            <Choice checked={prefs.hidden} onClick={() => update({ hidden: !prefs.hidden })}>
              显示隐藏文件
            </Choice>
            <Action onClick={startPathEdit}>前往文件夹…</Action>
          </Menu>
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
          <nav className="breadcrumbs" aria-label="当前位置">
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
                    <button onClick={() => x.navigate(p.path)}>{p.name}</button>
                  </span>
                ))}
              </>
            )}
          </nav>
          <button
            className="icon-button path-copy"
            title="复制路径"
            aria-label="复制路径"
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => copyPath()}
          >
            <Copy size={13} />
          </button>
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
        {x.query && (
          <div className="search-scope">
            <Search size={13} />
            <span>在“{displayName(x.path)}”及子文件夹中搜索</span>
            <label>
              <input
                type="checkbox"
                checked={prefs.respectIgnore}
                onChange={(e) => update({ respectIgnore: e.target.checked })}
              />
              遵循 .gitignore
            </label>
            <button onClick={() => update({ hidden: !prefs.hidden })}>
              {prefs.hidden ? '包含隐藏文件' : '不含隐藏文件'}
            </button>
          </div>
        )}
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
            onPreview={setPreview}
            onMenu={(entry, mx, my) => setMenu({ entry, x: mx, y: my })}
            onDrop={transfer}
          />
        )}
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
            left: Math.min(menu.x, innerWidth - 220),
            top: Math.max(8, Math.min(menu.y, innerHeight - 380)),
          }}
        >
          {[
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
              action: () => setPreview(menu.entry),
            },
            {
              label: '显示简介',
              icon: <Info />,
              hint: hint('info'),
              action: () => setInfo(menu.entry),
            },
            {
              label: '重新命名',
              icon: <Pencil />,
              hint: hint('rename'),
              action: () => x.beginRename(menu.entry.path),
            },
            { label: '复制', icon: <Copy />, hint: hint('copy'), action: () => copy(false) },
            { label: '剪切', icon: <Scissors />, hint: hint('cut'), action: () => copy(true) },
            { label: '创建副本', icon: <Copy />, hint: hint('duplicate'), action: duplicate },
            { label: '压缩为 ZIP', icon: <Archive />, hint: '', action: compress },
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
          ].map((a) => (
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
          ))}
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
      <FileInfoDialog info={info} onClose={() => setInfo(null)} onError={x.setError} />
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
