import { useEntryDecorations } from './useEntryDecorations'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useListingSearch } from './useListingSearch'
import { attachDirectoryWatch } from '@/features/explorer/model/directoryWatch'
import { PaintedListingCache } from '@/features/explorer/model/paintedListingCache'
import { canRenameEntry, renameSingleEntry } from '@/features/explorer/operations/rename'
import { useDirectorySnapshot, type DirectorySnapshotServices } from './useDirectorySnapshot'
import { useListingLocations } from './useListingLocations'
import type { EntryDecorationServices } from './useEntryDecorations'
import type { ListingLocationServices } from './useListingLocations'
import type { ListingSearchServices } from './useListingSearch'
import type { Entry, EntryDecoration, Preferences, RemoteConnection } from '@/lib/types'
import type { Navigation } from './useNavigation'

export interface ListingServices {
  native: boolean
  isVirtualResource: (path: string) => boolean
  requestDirectoryRefresh: (paths: string[]) => void
  listenRemoteSync: (onSync: () => void) => Promise<() => void>
  listenDirectoryChanged: (onChange: () => void) => Promise<() => void>
  listenApplicationsChanged: (onChange: () => void) => Promise<() => void>
  watchDirectory: (path: string, pane: string) => Promise<unknown>
  unwatchDirectory: (path: string, pane: string) => Promise<unknown>
  renameEntry: (path: string, name: string) => Promise<string>
  remoteConnections: () => Promise<RemoteConnection[]>
  watchErrorMessage: (error: unknown) => string
  renameUnsupportedMessage: () => string
  directorySnapshot: DirectorySnapshotServices
  decorations: EntryDecorationServices
  locations: ListingLocationServices
  search: ListingSearchServices
}

/** Everything a pane lists: entries, decorations, locations, connections,
 *  search results and current-directory capabilities. Path, history and tab
 *  state come from useNavigation. */
export function useListing(
  prefs: Preferences,
  nav: Navigation,
  services: ListingServices,
  enabled = true,
  pane = 'left',
  startOverride?: string,
) {
  const { path, query, setSelected, setRenaming, setError } = nav
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
    enabled: enabled && (Boolean(query) || !services.native),
    query,
    refresh: searchRefresh,
    services: services.decorations,
  })
  const decorations = query || !services.native ? searchDecorations : directoryDecorations
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [connectionsLoaded, setConnectionsLoaded] = useState(false)
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
    services: services.locations,
    ...(startOverride === undefined ? {} : { startOverride }),
  })
  const { searching, searchStats } = useListingSearch({
    enabled,
    pane,
    path,
    prefs,
    nav,
    services: services.search,
    refresh: searchRefresh,
    setEntries,
    setLoading,
  })
  const reload = useCallback(() => {
    if (query.trim()) setSearchRefresh((value) => value + 1)
    else services.requestDirectoryRefresh([path])
  }, [path, query, services])
  useEffect(() => {
    if (!services.native || !enabled) return
    const stop = services.listenRemoteSync(() => {
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
  }, [enabled, connections, path, reload, services])
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

  useDirectorySnapshot({
    enabled,
    path,
    query,
    prefs,
    revalidation,
    nav,
    locations,
    services: services.directorySnapshot,
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
      !services.native ||
      virtualVolume ||
      services.isVirtualResource(path) ||
      path.startsWith('fiex-plugin-view://')
    )
      return
    return attachDirectoryWatch({
      watch: () => services.watchDirectory(path, pane),
      listen: services.listenDirectoryChanged,
      unwatch: () => services.unwatchDirectory(path, pane),
      onChange: reload,
      onError: (error) => setWarnings([services.watchErrorMessage(error)]),
      debounceMs: 250,
    })
  }, [enabled, path, pane, reload, virtualVolume, services])
  useEffect(() => {
    if (!enabled || !services.native || !path.startsWith('applications://')) return
    let active = true
    let cleanup: (() => void) | undefined
    void services
      .listenApplicationsChanged(() => {
        if (query.trim()) setSearchRefresh((value) => value + 1)
        else setRevalidation((value) => value + 1)
      })
      .then((unlisten) => {
        if (active) cleanup = unlisten
        else unlisten()
      })
    return () => {
      active = false
      cleanup?.()
    }
  }, [enabled, path, query, services])
  useEffect(() => {
    setTagFilterState(null)
  }, [path])
  const beginRename = useCallback(
    (target: string, createdEntry?: Entry) => {
      const entry = entries.find((item) => item.path === target) ?? createdEntry
      if (entry?.path !== target || !canRenameEntry(entry)) return
      if (!entries.some((item) => item.path === target))
        setEntries((previous) => [...previous, entry])
      setSelected([target])
      setRenaming(target)
    },
    [entries, setRenaming, setSelected],
  )
  const cancelRename = useCallback(() => setRenaming(null), [setRenaming])
  const renameTo = useCallback(
    async (target: string, name: string) => {
      const result = await renameSingleEntry(target, name, entries, (resourcePath, nextName) =>
        services.renameEntry(resourcePath, nextName),
      )
      if (result.status === 'unsupported') {
        setError(services.renameUnsupportedMessage())
        return false
      }
      if (result.status === 'failed') {
        setError(result.message)
        return false
      }
      setError('')
      setRenaming(null)
      setSelected([result.path])
      services.requestDirectoryRefresh([path])
      return true
    },
    [entries, path, services, setError, setRenaming, setSelected],
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
      const list = await services.remoteConnections()
      setConnections(Array.isArray(list) ? [...list] : [])
    } catch {
      setConnections([])
    } finally {
      setConnectionsLoaded(true)
    }
  }, [services])
  useEffect(() => {
    if (!enabled) return
    void refreshConnections()
  }, [enabled, refreshConnections])
  return {
    entries: visibleEntries,
    decorations,
    connections,
    connectionsLoaded,
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
