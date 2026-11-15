import { useEffect } from 'react'
import { attachDirectoryWatch } from '@/features/explorer/model/directoryWatch'
import {
  connectionCoversPath,
  directoryChangeIncludesPath,
  listingRefreshTarget,
  shouldWatchDirectory,
} from '@/features/explorer/model/listingRefreshPolicy'
import type { RemoteConnection } from '@/lib/types'

export interface ListingRefreshSignalServices {
  native: boolean
  isVirtualResource: (path: string) => boolean
  listenRemoteSync: (onSync: () => void) => Promise<() => void>
  listenDirectoryChanged: (onChange: () => void) => Promise<() => void>
  listenApplicationsChanged: (onChange: () => void) => Promise<() => void>
  watchDirectory: (path: string, pane: string) => Promise<unknown>
  unwatchDirectory: (path: string, pane: string) => Promise<unknown>
  watchErrorMessage: (error: unknown) => string
}

interface ListingRefreshSignalOptions {
  enabled: boolean
  path: string
  query: string
  pane: string
  virtualVolume: boolean
  connections: RemoteConnection[]
  services: ListingRefreshSignalServices
  reload: () => void
  refreshSearch: () => void
  revalidateDirectory: () => void
  setWatchWarning: (warning: string) => void
}

export function useListingRefreshSignals({
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
}: ListingRefreshSignalOptions) {
  useEffect(() => {
    if (!services.native || !enabled) return
    const stop = services.listenRemoteSync(() => {
      if (!document.hidden && connectionCoversPath(path, connections)) reload()
    })
    return () => {
      void stop.then((unlisten) => unlisten())
    }
  }, [enabled, connections, path, reload, services])

  useEffect(() => {
    if (!enabled || !path) return
    const refresh = (event: Event) => {
      const detail = (event as CustomEvent<unknown>).detail
      if (!directoryChangeIncludesPath(detail, path)) return
      if (listingRefreshTarget(query) === 'search') refreshSearch()
      else revalidateDirectory()
    }
    window.addEventListener('fiex:directory-mutated', refresh)
    return () => window.removeEventListener('fiex:directory-mutated', refresh)
  }, [enabled, path, query, refreshSearch, revalidateDirectory])

  useEffect(() => {
    if (
      !shouldWatchDirectory(
        { enabled, native: services.native, path, virtualVolume },
        services.isVirtualResource,
      )
    )
      return
    return attachDirectoryWatch({
      watch: () => services.watchDirectory(path, pane),
      listen: services.listenDirectoryChanged,
      unwatch: () => services.unwatchDirectory(path, pane),
      onChange: reload,
      onError: (error) => setWatchWarning(services.watchErrorMessage(error)),
      debounceMs: 250,
    })
  }, [enabled, path, pane, reload, services, setWatchWarning, virtualVolume])

  useEffect(() => {
    if (!enabled || !services.native || !path.startsWith('applications://')) return
    let active = true
    let cleanup: (() => void) | undefined
    void services
      .listenApplicationsChanged(() => {
        if (listingRefreshTarget(query) === 'search') refreshSearch()
        else revalidateDirectory()
      })
      .then((unlisten) => {
        if (active) cleanup = unlisten
        else unlisten()
      })
    return () => {
      active = false
      cleanup?.()
    }
  }, [enabled, path, query, refreshSearch, revalidateDirectory, services])
}
