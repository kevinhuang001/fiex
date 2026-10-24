import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'
import {
  ArrowLeft,
  ArrowRight,
  ChevronUp,
  Folder,
  HardDrive,
  LayoutGrid,
  List,
  MoreHorizontal,
  Search,
  Server,
} from 'lucide-react'
import { call } from '@/lib/api'
import { useMenuPosition } from '@/lib/menuPosition'
import { FileIcon } from '@/components/FileIcon'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { usePreferences } from '@/hooks/usePreferences'
import { commonPlaces } from '@/lib/places'
import {
  currentFilePicker,
  settleFilePicker,
  subscribeFilePicker,
  type FilePickerRequest,
} from '@/lib/filePicker'
import { date, kind, size } from '@/lib/files'
import type { Entry, Listing, Location, RemoteConnection } from '@/lib/types'

import { PickerDialogs, type PickerDialogState } from './file-picker/PickerDialogs'
import { PickerMenu } from './file-picker/PickerMenu'
import {
  dirname,
  join,
  persistPickerSort,
  storedGroup,
  storedSort,
  type GroupKey,
} from './file-picker/types'

export function FilePickerDialog() {
  const pending = useSyncExternalStore(subscribeFilePicker, currentFilePicker, () => null)
  if (!pending) return null
  return (
    <Picker
      key={`${pending.request.mode}:${pending.request.directory || ''}`}
      request={pending.request}
    />
  )
}

function Picker({ request }: { request: FilePickerRequest }) {
  const [preferences] = usePreferences()
  const [directory, setDirectory] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const [locations, setLocations] = useState<Location[]>([])
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [selected, setSelected] = useState<string[]>([])
  const [history, setHistory] = useState<string[]>([])
  const [index, setIndex] = useState(-1)
  const [view, setView] = useState<'list' | 'grid'>('list')
  const [filter, setFilter] = useState('')
  const [saveName, setSaveName] = useState(request.suggestedName || '')
  const [replace, setReplace] = useState(false)
  const [reload, setReload] = useState(0)
  const [sort, setSort] = useState(storedSort)
  const [group, setGroup] = useState<GroupKey>(storedGroup)
  const [menu, setMenu] = useState<{
    x: number
    y: number
    entry?: Entry
    kind: 'row' | 'blank'
  } | null>(null)
  const [dialog, setDialog] = useState<PickerDialogState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const nameInput = useRef<HTMLInputElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const menuPosition = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, menuRef, Boolean(menu))
  const dialogInput = useRef<HTMLInputElement>(null)

  const navigate = (path: string) => {
    if (!path || path === directory) return
    setDirectory(path)
    setSelected([])
    setFilter('')
    setHistory((old) => [...old.slice(0, index + 1), path])
    setIndex((old) => old + 1)
  }

  useEffect(() => {
    void call<Location[]>('locations')
      .then((values) => setLocations(values || []))
      .catch(() => {})
    void call<RemoteConnection[]>('remote_connections')
      .then((values) => setConnections(values || []))
      .catch(() => {})
  }, [])

  useEffect(() => {
    let alive = true
    setError('')
    void call<Listing>('list_directory', { path: directory })
      .then((value) => {
        if (!alive) return
        setEntries(value.entries || [])
      })
      .catch((cause) => {
        if (alive) setError(String(cause))
      })
    return () => {
      alive = false
    }
  }, [directory, reload])

  useEffect(() => {
    if (!directory && request.directory) navigate(request.directory)
    if (!directory && !request.directory)
      void call<Location[]>('locations').then((values) => {
        const home = (values || []).find((item) => item.kind === 'home') || values?.[0]
        if (home && !directory) navigate(home.path)
      })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    if (request.mode !== 'saveFile') return
    // Radix focuses the dialog on mount, so focus the name field right after.
    const timer = window.setTimeout(() => {
      const input = nameInput.current
      if (!input) return
      input.focus()
      const dot = input.value.lastIndexOf('.')
      input.setSelectionRange(0, dot > 0 ? dot : input.value.length)
    }, 60)
    return () => window.clearTimeout(timer)
  }, [request.mode])

  useEffect(() => {
    if (!dialog) return
    const timer = window.setTimeout(() => {
      dialogInput.current?.focus()
      dialogInput.current?.select()
    }, 40)
    return () => window.clearTimeout(timer)
  }, [dialog])

  useEffect(() => {
    setReplace(false)
  }, [saveName, directory])

  useEffect(() => {
    persistPickerSort(sort, group)
  }, [sort, group])

  useEffect(() => {
    if (!menu) return
    const close = () => setMenu(null)
    const escape = (event: KeyboardEvent) => event.key === 'Escape' && setMenu(null)
    window.addEventListener('pointerdown', close)
    window.addEventListener('keydown', escape)
    return () => {
      window.removeEventListener('pointerdown', close)
      window.removeEventListener('keydown', escape)
    }
  }, [menu])

  const groupOf = (entry: Entry): { label: string; rank: number } => {
    if (group === 'kind')
      return entry.isDir ? { label: '文件夹', rank: 0 } : { label: kind(entry), rank: 1 }
    if (group === 'size') {
      if (entry.isDir) return { label: '文件夹', rank: 0 }
      if (entry.size === 0) return { label: '空', rank: 1 }
      if (entry.size < 1024 * 1024) return { label: '小于 1 MB', rank: 2 }
      if (entry.size < 100 * 1024 * 1024) return { label: '小于 100 MB', rank: 3 }
      return { label: '大于 100 MB', rank: 4 }
    }
    if (group === 'modified') {
      const value = entry.modified * 1000
      const now = Date.now()
      if (now - value < 24 * 60 * 60 * 1000) return { label: '今天', rank: 0 }
      if (now - value < 48 * 60 * 60 * 1000) return { label: '昨天', rank: 1 }
      if (now - value < 7 * 24 * 60 * 60 * 1000) return { label: '过去 7 天', rank: 2 }
      if (now - value < 30 * 24 * 60 * 60 * 1000) return { label: '过去 30 天', rank: 3 }
      return { label: '更早', rank: 4 }
    }
    return { label: '', rank: 0 }
  }

  const visible = useMemo(() => {
    const query = filter.trim().toLowerCase()
    const filtered = query
      ? entries.filter((entry) => entry.name.toLowerCase().includes(query))
      : entries
    const direction = sort.direction === 'asc' ? 1 : -1
    return [...filtered].sort((left, right) => {
      if (group !== 'none') {
        const leftGroup = groupOf(left)
        const rightGroup = groupOf(right)
        if (leftGroup.rank !== rightGroup.rank) return leftGroup.rank - rightGroup.rank
        if (leftGroup.label !== rightGroup.label)
          return leftGroup.label.localeCompare(rightGroup.label)
      }
      if (left.isDir !== right.isDir) return left.isDir ? -1 : 1
      const result =
        sort.key === 'name'
          ? left.name.localeCompare(right.name)
          : sort.key === 'modified'
            ? left.modified - right.modified
            : sort.key === 'size'
              ? left.size - right.size
              : kind(left).localeCompare(kind(right))
      return result * direction
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entries, filter, sort, group])

  const grouped = useMemo(() => {
    if (group === 'none') return [{ label: null as string | null, entries: visible }]
    const sections: { label: string | null; entries: Entry[] }[] = []
    for (const entry of visible) {
      const label = groupOf(entry).label
      const last = sections[sections.length - 1]
      if (last && last.label === label) last.entries.push(entry)
      else sections.push({ label, entries: [entry] })
    }
    return sections
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible, group])

  const selectedEntries = visible.filter((entry) => selected.includes(entry.path))
  const canConfirm =
    request.mode === 'saveFile'
      ? Boolean(saveName.trim()) && Boolean(directory)
      : request.mode === 'openDirectory'
        ? Boolean(directory)
        : selectedEntries.length > 0
  const target = entries.find((entry) => entry.name === saveName.trim() && !entry.isDir)

  const confirm = () => {
    if (!canConfirm) return
    if (request.mode === 'saveFile') {
      // Match Finder/Explorer: ask before replacing an existing file instead of
      // silently overwriting it.
      if (target && !replace) {
        setReplace(true)
        return
      }
      return settleFilePicker(join(directory, saveName.trim()))
    }
    if (request.mode === 'openDirectory') {
      const folder = selectedEntries.find((entry) => entry.isDir)
      return settleFilePicker(folder ? folder.path : directory)
    }
    const files = selectedEntries.filter((entry) => !entry.isDir)
    if (!files.length) return
    settleFilePicker(
      request.mode === 'openFiles' ? files.map((entry) => entry.path) : files[0].path,
    )
  }

  const openEntry = (entry: Entry) => {
    if (entry.isDir) return navigate(entry.path)
    if (request.mode === 'saveFile') {
      setSaveName(entry.name)
      setSelected([entry.path])
      return
    }
    settleFilePicker(request.mode === 'openFiles' ? [entry.path] : entry.path)
  }

  const toggle = (entry: Entry, multi: boolean) => {
    if (multi || request.mode === 'openFiles') {
      setSelected((old) =>
        old.includes(entry.path) ? old.filter((path) => path !== entry.path) : [...old, entry.path],
      )
    } else {
      setSelected([entry.path])
      if (request.mode === 'saveFile') setSaveName(entry.name)
    }
  }

  const perform = async (action: () => Promise<unknown>) => {
    setBusy(true)
    setError('')
    try {
      await action()
      setDialog(null)
      setMenu(null)
      setReload((value) => value + 1)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }

  const createFolder = (name: string) => {
    if (!name.trim()) return
    void perform(async () => {
      const path = await call<string>('create_entry', {
        path: directory,
        name: name.trim(),
        folder: true,
      })
      setSelected([path])
    })
  }
  const renameEntry = (entry: Entry, name: string) => {
    if (!name.trim() || name.trim() === entry.name) return
    void perform(() => call<string>('rename_entry', { path: entry.path, name: name.trim() }))
  }
  const trashEntry = (entry: Entry) => {
    void perform(() => call('recycle', { paths: [entry.path] }))
  }

  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (dialog || replace || menu) return
      if (event.key === 'Escape') {
        event.preventDefault()
        settleFilePicker(null)
      }
      if (event.key === 'Enter' && !(event.target as HTMLElement).closest('input')) {
        event.preventDefault()
        confirm()
      }
      if (event.key === 'Backspace' && !(event.target as HTMLElement).closest('input')) {
        event.preventDefault()
        navigate(dirname(directory))
      }
    }
    document.addEventListener('keydown', listener)
    return () => document.removeEventListener('keydown', listener)
  })

  const segments = directory.split('/').filter(Boolean)
  const places = commonPlaces(locations)
  const favorites = preferences.favorites || []
  const disks = locations.filter((location) => location.kind === 'disk')

  const contextMenu = (event: React.MouseEvent, entry?: Entry) => {
    event.preventDefault()
    event.stopPropagation()
    if (entry) setSelected([entry.path])
    setMenu({ x: event.clientX, y: event.clientY, entry, kind: entry ? 'row' : 'blank' })
  }

  return (
    <Modal
      open
      onClose={() => settleFilePicker(null)}
      title={
        request.mode === 'saveFile'
          ? '保存文件'
          : request.mode === 'openDirectory'
            ? '选择文件夹'
            : request.mode === 'openFiles'
              ? '选择文件'
              : '打开文件'
      }
      wide
      className="file-picker"
    >
      <div className="file-picker-body">
        <aside className="file-picker-sidebar">
          {places.length > 0 && <p className="file-picker-section">常用位置</p>}
          {places.map((item) => (
            <button
              key={`place:${item.kind}:${item.path || item.name}`}
              type="button"
              className={`file-picker-place ${directory === item.path ? 'active' : ''}`}
              onClick={() => navigate(item.path)}
            >
              <Folder size={14} />
              <span>{item.name}</span>
            </button>
          ))}
          <p className="file-picker-section">收藏</p>
          {!favorites.length && <p className="file-picker-empty">还没有收藏</p>}
          {favorites.map((item) => (
            <button
              key={`favorite:${item.path}`}
              type="button"
              className={`file-picker-place ${directory === item.path ? 'active' : ''}`}
              onClick={() => navigate(item.path)}
            >
              <Folder size={14} />
              <span>{item.name}</span>
            </button>
          ))}
          <p className="file-picker-section">连接</p>
          {!connections.length && <p className="file-picker-empty">没有连接</p>}
          {connections.map((item) => (
            <button
              key={`connection:${item.id}`}
              type="button"
              className={`file-picker-place ${directory === item.mountPath ? 'active' : ''}`}
              onClick={() => navigate(item.mountPath)}
            >
              <Server size={14} />
              <span>{item.name}</span>
            </button>
          ))}
          <p className="file-picker-section">设备</p>
          {!disks.length && <p className="file-picker-empty">没有设备</p>}
          {disks.map((item) => (
            <button
              key={`disk:${item.path || item.device}`}
              type="button"
              className={`file-picker-place ${directory === item.path ? 'active' : ''}`}
              onClick={() => item.path && navigate(item.path)}
            >
              <HardDrive size={14} />
              <span>{item.name}</span>
            </button>
          ))}
        </aside>
        <section className="file-picker-main">
          <div className="file-picker-toolbar">
            <button
              type="button"
              aria-label="后退"
              disabled={index <= 0}
              onClick={() => {
                const next = history[index - 1]
                setIndex(index - 1)
                setDirectory(next)
                setSelected([])
              }}
            >
              <ArrowLeft size={15} />
            </button>
            <button
              type="button"
              aria-label="前进"
              disabled={index >= history.length - 1}
              onClick={() => {
                const next = history[index + 1]
                setIndex(index + 1)
                setDirectory(next)
                setSelected([])
              }}
            >
              <ArrowRight size={15} />
            </button>
            <button type="button" aria-label="上一级" onClick={() => navigate(dirname(directory))}>
              <ChevronUp size={15} />
            </button>
            <nav className="file-picker-crumbs" aria-label="路径">
              {segments.length > 3 && (
                <button
                  type="button"
                  aria-label="更上层"
                  title="更上层"
                  onClick={() => navigate('/' + segments.slice(0, segments.length - 3).join('/'))}
                >
                  …
                </button>
              )}
              {segments.slice(Math.max(0, segments.length - 3)).map((segment, position) => {
                const index = segments.length - Math.min(3, segments.length) + position
                return (
                  <button
                    key={index}
                    type="button"
                    onClick={() => navigate('/' + segments.slice(0, index + 1).join('/'))}
                  >
                    {segment}
                  </button>
                )
              })}
            </nav>
            <button
              type="button"
              aria-label="更多"
              title="新建、排序、分组"
              onClick={(event) => {
                const rect = event.currentTarget.getBoundingClientRect()
                setMenu({ x: Math.max(8, rect.right - 190), y: rect.bottom + 4, kind: 'blank' })
              }}
            >
              <MoreHorizontal size={15} />
            </button>
            <label className="file-picker-search">
              <Search size={13} />
              <input
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
                placeholder="搜索"
                aria-label="搜索"
              />
            </label>
            <button
              type="button"
              aria-label={view === 'list' ? '图标视图' : '列表视图'}
              onClick={() => setView(view === 'list' ? 'grid' : 'list')}
            >
              {view === 'list' ? <LayoutGrid size={15} /> : <List size={15} />}
            </button>
          </div>
          <div
            className={`file-picker-entries ${view}`}
            role="listbox"
            aria-label="文件"
            onContextMenu={(event) => contextMenu(event)}
          >
            {error && <p className="file-picker-hint error">{error}</p>}
            {!error && !visible.length && <p className="file-picker-hint">此文件夹为空。</p>}
            {view === 'list'
              ? grouped.map((section) => (
                  <div key={section.label || 'all'}>
                    {section.label && <p className="file-picker-group">{section.label}</p>}
                    {section.entries.map((entry) => (
                      <button
                        key={entry.path}
                        type="button"
                        role="option"
                        aria-selected={selected.includes(entry.path)}
                        className={`file-picker-row ${selected.includes(entry.path) ? 'selected' : ''}`}
                        onClick={(event) =>
                          toggle(entry, event.metaKey || event.ctrlKey || event.shiftKey)
                        }
                        onDoubleClick={() => openEntry(entry)}
                        onContextMenu={(event) => contextMenu(event, entry)}
                      >
                        <FileIcon entry={entry} size={22} />
                        <span className="name">{entry.name}</span>
                        <span className="meta">{date(entry.modified)}</span>
                        <span className="meta size">{entry.isDir ? '—' : size(entry.size)}</span>
                      </button>
                    ))}
                  </div>
                ))
              : visible.map((entry) => (
                  <button
                    key={entry.path}
                    type="button"
                    role="option"
                    aria-selected={selected.includes(entry.path)}
                    className={`file-picker-tile ${selected.includes(entry.path) ? 'selected' : ''}`}
                    onClick={(event) =>
                      toggle(entry, event.metaKey || event.ctrlKey || event.shiftKey)
                    }
                    onDoubleClick={() => openEntry(entry)}
                    onContextMenu={(event) => contextMenu(event, entry)}
                  >
                    <FileIcon entry={entry} size={44} />
                    <span>{entry.name}</span>
                  </button>
                ))}
          </div>
        </section>
      </div>
      {menu &&
        createPortal(
          <PickerMenu
            menu={menu}
            sort={sort}
            group={group}
            view={view}
            position={menuPosition}
            menuRef={menuRef}
            actions={{
              open: openEntry,
              rename: (entry) => setDialog({ kind: 'rename', entry, value: entry.name }),
              trash: (entry) => setDialog({ kind: 'trash', entry }),
              newFolder: () => setDialog({ kind: 'folder', value: '未命名文件夹' }),
              refresh: () => setReload((value) => value + 1),
              toggleView: () => setView(view === 'list' ? 'grid' : 'list'),
              sortBy: (key) =>
                setSort((old) =>
                  old.key === key
                    ? { key, direction: old.direction === 'asc' ? 'desc' : 'asc' }
                    : { key, direction: 'asc' },
                ),
              groupBy: setGroup,
              close: () => setMenu(null),
            }}
          />,
          document.body,
        )}
      <PickerDialogs
        dialog={dialog}
        replace={replace}
        saveName={saveName}
        busy={busy}
        inputRef={dialogInput}
        actions={{
          closeDialog: () => setDialog(null),
          setDialogValue: (value) =>
            setDialog((old) => (old && old.kind !== 'trash' ? { ...old, value } : old)),
          createFolder,
          rename: renameEntry,
          trash: trashEntry,
          confirmReplace: () => settleFilePicker(join(directory, saveName.trim())),
          cancelReplace: () => setReplace(false),
        }}
      />
      <footer className="file-picker-footer">
        {request.mode === 'saveFile' ? (
          <label className="file-picker-name">
            <span>存储为</span>
            <input
              ref={nameInput}
              value={saveName}
              onChange={(event) => setSaveName(event.target.value)}
              placeholder="文件名"
              aria-label="文件名"
            />
          </label>
        ) : (
          <span className="file-picker-selection">
            {selectedEntries.length
              ? selectedEntries.map((entry) => entry.name).join('、')
              : '未选择'}
          </span>
        )}
        <div className="file-picker-actions">
          <Button variant="outline" onClick={() => settleFilePicker(null)}>
            取消
          </Button>
          <Button disabled={!canConfirm} onClick={confirm}>
            {request.mode === 'saveFile' ? '保存' : '打开'}
          </Button>
        </div>
      </footer>
    </Modal>
  )
}
