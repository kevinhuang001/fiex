import { useCallback, useEffect, useState } from 'react'
import { resolveInitialPathRequest } from '@/features/explorer/model/initialPath'
import { createLocationRefreshCoordinator } from '@/features/explorer/model/locationRefresh'
import type { Location, Preferences } from '@/lib/types'

export interface ListingLocationServices {
  native: boolean
  loadLocations: () => Promise<Location[]>
  loadLaunchPath: () => Promise<string | null>
  loadLastPath: () => Promise<string | null>
  loadInitialPath: () => Promise<string | null>
  persistLastPath: (path: string) => Promise<void>
}

interface ListingLocationsNavigation {
  startAt: (path: string) => void
}

/** Sidebar locations and the asynchronous start-folder resolution for a pane. */
export function useListingLocations({
  enabled,
  pane,
  path,
  query,
  listedPath,
  prefs,
  nav,
  services,
  startOverride,
}: {
  enabled: boolean
  pane: string
  path: string
  query: string
  listedPath: string
  prefs: Pick<Preferences, 'startPage'>
  nav: ListingLocationsNavigation
  services: ListingLocationServices
  startOverride?: string
}) {
  const { startAt } = nav
  const [locations, setLocations] = useState<Location[]>([])
  const [locationRefresh] = useState(() =>
    createLocationRefreshCoordinator(services.loadLocations, setLocations),
  )
  const refreshLocations = useCallback(
    (force = false) => locationRefresh.refresh(force),
    [locationRefresh],
  )

  useEffect(() => {
    if (!enabled || pane !== 'left') return
    const check = () => {
      if (!document.hidden) void refreshLocations().catch(() => {})
    }
    const timer = window.setInterval(check, 10000)
    window.addEventListener('focus', check)
    document.addEventListener('visibilitychange', check)
    return () => {
      window.clearInterval(timer)
      window.removeEventListener('focus', check)
      document.removeEventListener('visibilitychange', check)
    }
  }, [refreshLocations, enabled, pane])

  useEffect(() => {
    if (!enabled || path) return
    let cancelled = false
    // Resolve the first directory without waiting for the full location survey.
    void refreshLocations().catch(() => {})
    void resolveInitialPathRequest({
      ...(startOverride === undefined ? {} : { startOverride }),
      startPage: prefs.startPage,
      native: services.native,
      isCancelled: () => cancelled,
      loadLaunchPath: services.loadLaunchPath,
      loadLastPath: services.loadLastPath,
      loadInitialPath: services.loadInitialPath,
      loadLocations: services.loadLocations,
    }).then((start) => {
      if (start && !cancelled) startAt(start)
    })
    return () => {
      cancelled = true
    }
  }, [refreshLocations, enabled, path, prefs.startPage, startOverride, startAt, services])

  useEffect(() => {
    if (!enabled || !path || query || listedPath !== path) return
    void services.persistLastPath(path)
  }, [enabled, path, listedPath, query, services])

  return { locations, refreshLocations }
}
