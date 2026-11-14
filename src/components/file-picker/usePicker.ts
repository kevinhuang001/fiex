import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent } from 'react'
import { useMenuPosition } from '@/lib/menuPosition'
import { usePreferences } from '@/hooks/usePreferences'
import { commonPlaces } from '@/lib/places'
import type { FilePickerRequest } from '@/lib/filePicker'
import type { Entry } from '@/lib/types'

import type { PickerDialogState } from './PickerDialogs'
import type { PickerMenuState } from './PickerMenu'
import { dirname, type GroupKey, type SortKey } from './types'
import { groupPickerEntries, visiblePickerEntries } from './model'
import { usePickerDirectory } from './usePickerDirectory'
import { usePickerSelection } from './usePickerSelection'

/** Owns every piece of picker state and every side effect so the dialog shell
 * stays a thin composition root. */
export function usePicker(request: FilePickerRequest) {
  const [preferences] = usePreferences()
  const [selected, setSelected] = useState<string[]>([])
  const [view, setView] = useState<'list' | 'grid'>('list')
  const [filter, setFilter] = useState('')
  const [saveName, setSaveName] = useState(request.suggestedName || '')
  const [replace, setReplace] = useState(false)
  const [menu, setMenu] = useState<PickerMenuState | null>(null)
  const [dialog, setDialog] = useState<PickerDialogState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const directoryState = usePickerDirectory(request, setError)
  const {
    directory,
    setDirectory,
    entries,
    loading,
    locations,
    connections,
    history,
    setIndex,
    index,
    sort,
    setSort,
    group,
    setGroup,
    setReload,
    navigate: navigateDirectory,
  } = directoryState
  const nameInput = useRef<HTMLInputElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const menuPosition = useMenuPosition(menu?.x ?? 0, menu?.y ?? 0, menuRef, Boolean(menu))
  const dialogInput = useRef<HTMLInputElement>(null)

  const navigate = (path: string) => {
    if (navigateDirectory(path)) {
      setSelected([])
      setFilter('')
    }
  }

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

  const visible = useMemo(
    () => visiblePickerEntries(entries, filter, request.mode, request.extensions, sort, group),
    [entries, filter, sort, group, request.mode, request.extensions, getLocale()],
  )
  const grouped = useMemo(() => groupPickerEntries(visible, group), [visible, group, getLocale()])
  const selection = usePickerSelection({
    request,
    directory,
    entries,
    visible,
    selected,
    setSelected,
    saveName,
    setSaveName,
    replace,
    setReplace,
    setDialog,
    setMenu,
    setReload,
    setBusy,
    setError,
    navigate,
  })
  const {
    selectedEntries,
    canConfirm,
    confirm,
    openEntry,
    toggle,
    createFolder,
    renameEntry,
    trashEntry,
    confirmReplace,
    cancelReplace,
    cancel,
  } = selection

  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (dialog || replace || menu) return
      if (event.key === 'Escape') {
        event.preventDefault()
        cancel()
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
    loading,
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
