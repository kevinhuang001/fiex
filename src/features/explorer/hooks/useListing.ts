import { useEntryDecorations } from './useEntryDecorations'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useListingSearch } from './useListingSearch'
import { PaintedListingCache } from '@/features/explorer/model/paintedListingCache'
import {
  useInlineRename,
  type InlineRenameServices,
} from '@/features/file-operations/hooks/useInlineRename'
import { useDirectorySnapshot, type DirectorySnapshotServices } from './useDirectorySnapshot'
import { useListingLocations } from './useListingLocations'
import type { EntryDecorationServices } from './useEntryDecorations'
import type { ListingLocationServices } from './useListingLocations'
import type { ListingSearchServices } from './useListingSearch'
import type { Entry, EntryDecoration, Preferences, RemoteConnection } from '@/lib/types'
import type { Navigation } from './useNavigation'
import { useListingRefreshSignals } from './useListingRefreshSignals'

export interface ListingServices {
  native: boolean
  isVirtualResource: (path: string) => boolean
  requestDirectoryRefresh: (paths: string[]) => void
  listenRemoteSync: (onSync: () => void) => Promise<() => void>
  listenDirectoryChanged: (onChange: () => void) => Promise<() => void>
  listenApplicationsChanged: (onChange: () => void) => Promise<() => void>
  watchDirectory: (path: string, pane: string) => Promise<unknown>
  unwatchDirectory: (path: string, pane: string) => Promise<unknown>
  inlineRename: InlineRenameServices
  watchErrorMessage: (error: unknown) => string
  directorySnapshot: DirectorySnapshotServices
  decorations: EntryDecorationServices
  locations: ListingLocationServices
  search: ListingSearchServices
}

/** Everything a pane lists: entries, decorations, locations, search results
 *  and current-directory capabilities. Shared connection state is app-owned. */
export function useListing(
  prefs: Preferences,
  nav: Navigation,
  services: ListingServices,
  connections: RemoteConnection[],
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
  const refreshSearch = useCallback(() => setSearchRefresh((value) => value + 1), [])
  const revalidateDirectory = useCallback(() => setRevalidation((value) => value + 1), [])
  const setWatchWarning = useCallback((message: string) => setWarnings([message]), [setWarnings])
  const { beginRename, cancelRename, renameTo } = useInlineRename({
    path,
    entries,
    setEntries,
    setSelected,
    setRenaming,
    setError,
    refreshDirectory: services.requestDirectoryRefresh,
    services: services.inlineRename,
  })
  useListingRefreshSignals({
    enabled,
    path,
    query,
    pane,
    virtualVolume,
    connections,
    services,
    reload,
    refreshSearch,
    revalidateDirectory,
    setWatchWarning,
  })

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
    setTagFilterState(null)
  }, [path])
  const pathEntries =
    renderedPath === path ? entries : (paintedByPath.current.get(viewKey)?.listing.entries ?? [])
  const visibleEntries = tagFilter
    ? pathEntries.filter((entry) => entry.tags.some((tag) => tag.name === tagFilter))
    : pathEntries
  const setTagFilter = (tag: string | null) => {
    setTagFilterState(tag)
    setSelected([])
  }
  return {
    entries: visibleEntries,
    decorations,
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
