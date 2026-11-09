import { useEntryDecorations } from './useEntryDecorations'
import { t, getLocale } from '@/i18n'
import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { call, native } from '@/lib/api'
import { requestDirectoryRefresh } from '@/lib/listing'
import { isVirtualResource } from '@/lib/files'
import { useListingSearch } from './useListingSearch'
import { PaintedListingCache } from './paintedListingCache'
import { useDirectorySnapshot } from './useDirectorySnapshot'
import { useListingLocations } from './useListingLocations'
import type { Entry, EntryDecoration, Preferences, RemoteConnection } from '@/lib/types'
import type { Navigation } from './useNavigation'

/** Everything a pane lists: entries, decorations, locations, connections,
 *  search results and current-directory capabilities. Path, history and tab
 *  state come from useNavigation. */
export function useListing(
  prefs: Preferences,
  nav: Navigation,
  enabled = true,
  pane = 'left',
  startOverride?: string,
) {
  const {
    path,
    query,
    setSelected,
    pendingSelection,
    clearPendingSelection,
    setRenaming,
    setError,
  } = nav
  const viewKey = JSON.stringify([path, prefs.hidden, prefs.displayIgnore])
  const currentViewKeyRef = useRef(viewKey)
  currentViewKeyRef.current = viewKey
  const [entries, setEntries] = useState<Entry[]>([])
  const entriesRef = useRef(entries)
  entriesRef.current = entries
  const [tagFilter, setTagFilterState] = useState<string | null>(null)
  const [directoryAllowedOps, setDirectoryAllowedOps] = useState<string[]>([])
  const [virtualVolume, setVirtualVolume] = useState(false)
  const [defaultView, setDefaultView] = useState<'list' | 'icons' | 'gallery' | 'columns' | null>(
    null,
  )
  const [loading, setLoading] = useState(false)
  const [revalidation, setRevalidation] = useState(0)
  const [searchRefresh, setSearchRefresh] = useState(0)
  const [warnings, setWarnings] = useState<string[]>([])
  const [directoryDecorations, setDirectoryDecorations] = useState<Record<string, EntryDecoration>>(
    {},
  )
  const searchDecorations = useEntryDecorations({
    entries,
    path,
    enabled: enabled && (Boolean(query) || !native),
    query,
    refresh: searchRefresh,
  })
  const decorations = query || !native ? searchDecorations : directoryDecorations
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [renderedPath, setRenderedPath] = useState('')
  const paintedByPath = useRef(new PaintedListingCache(32))
  const renderedPathRef = useRef('')
  const renderedKeyRef = useRef('')
  const request = useRef(0)
  const [listedPath, setListedPath] = useState('')
  const { locations, refreshLocations } = useListingLocations({
    enabled,
    pane,
    path,
    query,
    listedPath,
    prefs,
    nav,
    startOverride,
  })
  const { searching, searchStats } = useListingSearch({
    enabled,
    pane,
    path,
    prefs,
    nav,
    refresh: searchRefresh,
    setEntries,
    setLoading,
  })
  const reload = useCallback(() => {
    if (query.trim()) setSearchRefresh((value) => value + 1)
    else requestDirectoryRefresh([path])
  }, [path, query])
  useEffect(() => {
    if (!native || !enabled) return
    const stop = listen('remote-sync', () => {
      if (
        !document.hidden &&
        connections.some(
          (c) => path.startsWith(c.mountPath.replace(/\/$/, '') + '/') || path === c.mountPath,
        )
      )
        reload()
    })
    return () => {
      void stop.then((unlisten) => unlisten())
    }
  }, [enabled, connections, path, reload])
  useEffect(() => {
    if (!enabled || !path) return
    const refresh = (event: Event) => {
      const directories = (event as CustomEvent<{ directories?: unknown }>).detail?.directories
      if (!Array.isArray(directories) || !directories.includes(path)) return
      if (query.trim()) setSearchRefresh((value) => value + 1)
      else setRevalidation((value) => value + 1)
    }
    window.addEventListener('fiex:directory-mutated', refresh)
    return () => window.removeEventListener('fiex:directory-mutated', refresh)
  }, [enabled, path, query])

  useEffect(() => {
    if (!pendingSelection || pendingSelection.directory !== path || listedPath !== path) return
    if (entries.some((entry) => entry.path === pendingSelection.target)) {
      setSelected([pendingSelection.target])
      clearPendingSelection(pendingSelection.generation)
    }
  }, [pendingSelection, path, listedPath, entries, setSelected, clearPendingSelection])

  useDirectorySnapshot({
    enabled,
    path,
    query,
    prefs,
    revalidation,
    nav,
    locations,
    state: {
      currentViewKey: currentViewKeyRef,
      entries: entriesRef,
      paintedByPath,
      renderedKey: renderedKeyRef,
      renderedPathRef,
      request,
      setEntries,
      setDirectoryAllowedOps,
      setVirtualVolume,
      setDefaultView,
      setWarnings,
      setDirectoryDecorations,
      setListedPath,
      setRenderedPath,
      setLoading,
      setError,
    },
  })
  useEffect(() => {
    if (
      !enabled ||
      !path ||
      !native ||
      virtualVolume ||
      isVirtualResource(path) ||
      path.startsWith('fiex-plugin-view://')
    )
      return
    let cleanup: (() => void) | undefined
    let active = true
    let timer: ReturnType<typeof setTimeout>
    void call('watch_directory', { path, pane }).catch((e) =>
      setWarnings([t('无法监视文件变更：{0}', e)]),
    )
    void listen('directory-changed', () => {
      clearTimeout(timer)
      timer = setTimeout(reload, 250)
    }).then((fn) => {
      if (active) cleanup = fn
      else fn()
    })
    return () => {
      active = false
      clearTimeout(timer)
      cleanup?.()
      void call('unwatch_directory', { path, pane }).catch(() => {})
    }
  }, [enabled, path, pane, reload, virtualVolume])
  useEffect(() => {
    if (!enabled || !native || !path.startsWith('applications://')) return
    let active = true
    let cleanup: (() => void) | undefined
    void listen('applications-changed', () => {
      if (query.trim()) setSearchRefresh((value) => value + 1)
      else setRevalidation((value) => value + 1)
    }).then((unlisten) => {
      if (active) cleanup = unlisten
      else unlisten()
    })
    return () => {
      active = false
      cleanup?.()
    }
  }, [enabled, native, path, query])
  useEffect(() => {
    setTagFilterState(null)
  }, [path])
  const beginRename = useCallback(
    (target: string, createdEntry?: Entry) => {
      const entry = entries.find((item) => item.path === target) ?? createdEntry
      if (entry?.path !== target || !entry.allowedOps?.includes('rename')) return
      if (!entries.some((item) => item.path === target))
        setEntries((previous) => [...previous, entry])
      setSelected([target])
      setRenaming(target)
    },
    [entries, getLocale()],
  )
  const cancelRename = useCallback(() => setRenaming(null), [getLocale()])
  const renameTo = useCallback(
    async (target: string, name: string) => {
      if (!entries.find((entry) => entry.path === target)?.allowedOps?.includes('rename')) {
        setError(t('此位置不支持该操作'))
        return false
      }
      setError('')
      try {
        const next = await call('rename_entry', { path: target, name })
        setRenaming(null)
        setSelected([next])
        requestDirectoryRefresh([path])
        return true
      } catch (e) {
        setError(String(e))
        return false
      }
    },
    [entries, getLocale(), path],
  )
  const pathEntries =
    renderedPath === path ? entries : (paintedByPath.current.get(viewKey)?.listing.entries ?? [])
  const visibleEntries = tagFilter
    ? pathEntries.filter((entry) => entry.tags.some((tag) => tag.name === tagFilter))
    : pathEntries
  const setTagFilter = (tag: string | null) => {
    setTagFilterState(tag)
    setSelected([])
  }
  const refreshConnections = useCallback(async () => {
    try {
      const list = await call('remote_connections')
      setConnections(Array.isArray(list) ? [...list] : [])
    } catch {
      setConnections([])
    }
  }, [getLocale()])
  useEffect(() => {
    if (!enabled) return
    void refreshConnections()
  }, [enabled, refreshConnections])
  return {
    entries: visibleEntries,
    decorations,
    connections,
    refreshConnections,
    tagFilter,
    setTagFilter,
    directoryAllowedOps,
    virtualVolume,
    defaultView,
    locations,
    refreshLocations,
    loading,
    listedPath,
    warnings,
    searching,
    searchStats,
    reload,
    beginRename,
    cancelRename,
    renameTo,
  }
}
