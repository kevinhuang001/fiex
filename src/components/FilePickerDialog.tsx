import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import {
  ArrowLeft,
  ArrowRight,
  ChevronUp,
  Folder,
  HardDrive,
  LayoutGrid,
  List,
  Search,
  Server,
} from 'lucide-react'
import { call } from '@/lib/api'
import { usePreferences } from '@/hooks/usePreferences'
import { commonPlaces } from '@/lib/places'
import { FileIcon } from '@/components/FileIcon'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import {
  currentFilePicker,
  settleFilePicker,
  subscribeFilePicker,
  type FilePickerRequest,
} from '@/lib/filePicker'
import { date, size } from '@/lib/files'
import type { Entry, Listing, Location, RemoteConnection } from '@/lib/types'

const dirname = (path: string) => {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index > 0 ? path.slice(0, index) : path
}
const join = (directory: string, name: string) =>
  `${directory.replace(/[/\\]$/, '')}/${name}`.replace(/\/+/g, '/')

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
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const list = useRef<HTMLDivElement>(null)
  const nameInput = useRef<HTMLInputElement>(null)

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
    setLoading(true)
    setError('')
    void call<Listing>('list_directory', { path: directory })
      .then((value) => {
        if (!alive) return
        setEntries(
          (value.entries || [])
            .filter((entry) => !entry.name.startsWith('.'))
            .sort((a, b) => Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name)),
        )
      })
      .catch((cause) => {
        if (alive) setError(String(cause))
      })
      .finally(() => {
        if (alive) setLoading(false)
      })
    return () => {
      alive = false
    }
  }, [directory])

  useEffect(() => {
    if (request.mode !== 'saveFile') return
    // Radix focuses the dialog on mount, so focus the name field right after.
    const timer = window.setTimeout(() => {
      const input = nameInput.current
      if (!input) return
      input.focus()
      // Select the basename so typing replaces it, like Finder's save panel.
      const dot = input.value.lastIndexOf('.')
      input.setSelectionRange(0, dot > 0 ? dot : input.value.length)
    }, 60)
    return () => window.clearTimeout(timer)
  }, [request.mode])

  useEffect(() => {
    if (!directory && request.directory) navigate(request.directory)
    if (!directory && !request.directory)
      void call<Location[]>('locations').then((values) => {
        const home = (values || []).find((item) => item.kind === 'home') || values?.[0]
        if (home && !directory) navigate(home.path)
      })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const visible = useMemo(() => {
    const query = filter.trim().toLowerCase()
    return query ? entries.filter((entry) => entry.name.toLowerCase().includes(query)) : entries
  }, [entries, filter])

  const selectedEntries = visible.filter((entry) => selected.includes(entry.path))
  const canConfirm =
    request.mode === 'saveFile'
      ? Boolean(saveName.trim()) && Boolean(directory)
      : request.mode === 'openDirectory'
        ? Boolean(directory)
        : selectedEntries.length > 0 &&
          (request.mode === 'openFiles' || !selectedEntries[0].isDir || true)

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

  useEffect(() => {
    setReplace(false)
  }, [saveName, directory])

  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        settleFilePicker(null)
      }
      if (event.key === 'Enter') {
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
              {segments.map((segment, position) => (
                <button
                  key={position}
                  type="button"
                  onClick={() => navigate('/' + segments.slice(0, position + 1).join('/'))}
                >
                  {segment}
                </button>
              ))}
            </nav>
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
            ref={list}
            role="listbox"
            aria-label="文件"
          >
            {loading && <p className="file-picker-hint">正在读取…</p>}
            {error && <p className="file-picker-hint error">{error}</p>}
            {!loading && !error && visible.length === 0 && (
              <p className="file-picker-hint">此文件夹为空。</p>
            )}
            {view === 'list'
              ? visible.map((entry) => (
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
                  >
                    <FileIcon entry={entry} size={22} />
                    <span className="name">{entry.name}</span>
                    <span className="meta">{date(entry.modified)}</span>
                    <span className="meta size">{entry.isDir ? '—' : size(entry.size)}</span>
                  </button>
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
                  >
                    <FileIcon entry={entry} size={44} />
                    <span>{entry.name}</span>
                  </button>
                ))}
          </div>
        </section>
      </div>
      <Modal
        open={replace}
        onClose={() => setReplace(false)}
        title="替换文件？"
        className="file-picker-confirm"
      >
        <p className="file-picker-confirm-text">
          「{saveName.trim()}」已存在。替换会覆盖原文件，此操作不可撤销。
        </p>
        <footer className="file-picker-confirm-actions">
          <Button variant="outline" onClick={() => setReplace(false)}>
            取消
          </Button>
          <Button onClick={() => settleFilePicker(join(directory, saveName.trim()))}>替换</Button>
        </footer>
      </Modal>
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
