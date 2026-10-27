import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent } from 'react'
import { call } from '@/lib/api'
import { listDirectory } from '@/lib/listing'
import { useMenuPosition } from '@/lib/menuPosition'
import { usePreferences } from '@/hooks/usePreferences'
import { commonPlaces } from '@/lib/places'
import { settleFilePicker, type FilePickerRequest } from '@/lib/filePicker'
import { loadPickerSidebarData, pickerSidebarSnapshot } from '@/lib/filePickerData'
import { kind } from '@/lib/files'
import type { Entry, Location, RemoteConnection } from '@/lib/types'

import type { PickerDialogState } from './PickerDialogs'
import type { PickerMenuState } from './PickerMenu'
import {
  dirname,
  join,
  persistPickerSort,
  storedGroup,
  storedSort,
  type GroupKey,
  type SortKey,
} from './types'

export interface PickerGroup {
  label: string | null
  entries: Entry[]
}

/** Owns every piece of picker state and every side effect so the dialog shell
 * stays a thin composition root. */
export function usePicker(request: FilePickerRequest) {
  const [preferences] = usePreferences()
  const [directory, setDirectory] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const initialSidebar = pickerSidebarSnapshot()
  const [locations, setLocations] = useState<Location[]>(initialSidebar?.locations || [])
  const [connections, setConnections] = useState<RemoteConnection[]>(() =>
    (initialSidebar?.connections || []).filter((item) =>
      item.mountPath.startsWith('fiex://mount/'),
    ),
  )
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
  const [menu, setMenu] = useState<PickerMenuState | null>(null)
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
    let alive = true
    void loadPickerSidebarData()
      .then(({ locations, connections }) => {
        if (!alive) return
        setLocations(locations)
        // A picker can only navigate VFS-backed connections.
        setConnections(connections.filter((item) => item.mountPath.startsWith('fiex://mount/')))
        if (!directory) {
          const target =
            request.directory ||
            (locations.find((item) => item.kind === 'home') || locations[0])?.path ||
            ''
          if (target) {
            setDirectory(target)
            setHistory([target])
            setIndex(0)
          }
        }
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [])

  useEffect(() => {
    let alive = true
    setError('')
    void listDirectory(directory)
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
    const close = (event: PointerEvent) => {
      // A pointerdown on a menu item must reach its click handler; closing the
      // menu here would unmount the button before the click fires.
      if (!menuRef.current?.contains(event.target as Node)) setMenu(null)
    }
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
      return entry.isDir ? { label: t('文件夹'), rank: 0 } : { label: kind(entry), rank: 1 }
    if (group === 'size') {
      if (entry.isDir) return { label: t('文件夹'), rank: 0 }
      if (entry.size === 0) return { label: t('空'), rank: 1 }
      if (entry.size < 1024 * 1024) return { label: t('小于 1 MB'), rank: 2 }
      if (entry.size < 100 * 1024 * 1024) return { label: t('小于 100 MB'), rank: 3 }
      return { label: t('大于 100 MB'), rank: 4 }
    }
    if (group === 'modified') {
      const value = entry.modified * 1000
      const now = Date.now()
      if (now - value < 24 * 60 * 60 * 1000) return { label: t('今天'), rank: 0 }
      if (now - value < 48 * 60 * 60 * 1000) return { label: t('昨天'), rank: 1 }
      if (now - value < 7 * 24 * 60 * 60 * 1000) return { label: t('过去 7 天'), rank: 2 }
      if (now - value < 30 * 24 * 60 * 60 * 1000) return { label: t('过去 30 天'), rank: 3 }
      return { label: t('更早'), rank: 4 }
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
  }, [entries, filter, sort, group, getLocale()])

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
  }, [visible, group, getLocale()])

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

  const back = () => {
    const next = history[index - 1]
    setIndex(index - 1)
    setDirectory(next)
    setSelected([])
  }
  const forward = () => {
    const next = history[index + 1]
    setIndex(index + 1)
    setDirectory(next)
    setSelected([])
  }
  const up = () => navigate(dirname(directory))
  const toggleView = () => setView(view === 'list' ? 'grid' : 'list')
  const openBlankMenu = (event: ReactMouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    setMenu({ x: Math.max(8, rect.right - 190), y: rect.bottom + 4, kind: 'blank' })
  }
  const sortBy = (key: SortKey) =>
    setSort((old) =>
      old.key === key
        ? { key, direction: old.direction === 'asc' ? 'desc' : 'asc' }
        : { key, direction: 'asc' },
    )
  const groupBy = (key: GroupKey) => setGroup(key)
  const refresh = () => setReload((value) => value + 1)
  const closeMenu = () => setMenu(null)
  const requestRename = (entry: Entry) => setDialog({ kind: 'rename', entry, value: entry.name })
  const requestTrash = (entry: Entry) => setDialog({ kind: 'trash', entry })
  const requestNewFolder = () => setDialog({ kind: 'folder', value: t('未命名文件夹') })
  const closeDialog = () => setDialog(null)
  const setDialogValue = (value: string) =>
    setDialog((old) => (old && old.kind !== 'trash' ? { ...old, value } : old))
  const confirmReplace = () => settleFilePicker(join(directory, saveName.trim()))
  const cancelReplace = () => setReplace(false)
  const cancel = () => settleFilePicker(null)

  const places = commonPlaces(locations).filter(
    (item) => item.kind !== 'applications' && item.kind !== 'trash',
  )
  const favorites = preferences.favorites || []
  const disks = locations.filter((location) => location.kind === 'disk')

  const contextMenu = (event: ReactMouseEvent, entry?: Entry) => {
    event.preventDefault()
    event.stopPropagation()
    if (entry) setSelected([entry.path])
    setMenu({ x: event.clientX, y: event.clientY, entry, kind: entry ? 'row' : 'blank' })
  }

  return {
    directory,
    selected,
    selectedEntries,
    visible,
    grouped,
    error,
    busy,
    view,
    filter,
    setFilter,
    saveName,
    setSaveName,
    replace,
    sort,
    group,
    history,
    index,
    canConfirm,
    places,
    favorites,
    connections,
    disks,
    menu,
    menuRef,
    menuPosition,
    dialog,
    dialogInput,
    nameInput,
    navigate,
    back,
    forward,
    up,
    toggleView,
    openBlankMenu,
    confirm,
    openEntry,
    toggle,
    contextMenu,
    createFolder,
    renameEntry,
    trashEntry,
    requestRename,
    requestTrash,
    requestNewFolder,
    closeMenu,
    refresh,
    sortBy,
    groupBy,
    closeDialog,
    setDialogValue,
    confirmReplace,
    cancelReplace,
    cancel,
  }
}
