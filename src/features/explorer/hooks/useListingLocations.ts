import { useCallback, useEffect, useState } from 'react'
import { call, native } from '@/lib/api'
import { resolveInitialPathRequest } from '@/features/explorer/model/initialPath'
import { createLocationRefreshCoordinator } from '@/features/explorer/model/locationRefresh'
import type { Location, Preferences } from '@/lib/types'
import type { Navigation } from './useNavigation'

/** Sidebar locations and the asynchronous start-folder resolution for a pane. */
export function useListingLocations({
  enabled,
  pane,
  path,
  query,
  listedPath,
  prefs,
  nav,
  startOverride,
}: {
  enabled: boolean
  pane: string
  path: string
  query: string
  listedPath: string
  prefs: Preferences
  nav: Navigation
  startOverride?: string
}) {
  const { startAt } = nav
  const [locations, setLocations] = useState<Location[]>([])
  const [locationRefresh] = useState(() =>
    createLocationRefreshCoordinator(() => call('locations'), setLocations),
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
      startOverride,
      startPage: prefs.startPage,
      native,
      isCancelled: () => cancelled,
      loadLaunchPath: () => call('launch_path'),
      loadLastPath: async () => {
        const lastPath = await call('state_get', {
          namespace: 'explorer',
          key: 'lastPath',
        }).catch(() => null)
        return lastPath || localStorage.getItem('fiex:lastPath')
      },
      loadInitialPath: () => call('initial_path'),
      loadLocations: () => call('locations'),
    }).then((start) => {
      if (start && !cancelled) startAt(start)
    })
    return () => {
      cancelled = true
    }
  }, [refreshLocations, enabled, path, prefs.startPage, startOverride, startAt])

  useEffect(() => {
    if (!enabled || !path || query || listedPath !== path) return
    void call('state_put', {
      namespace: 'explorer',
      key: 'lastPath',
      value: path,
    })
      .then(() => localStorage.removeItem('fiex:lastPath'))
      .catch(() => localStorage.setItem('fiex:lastPath', path))
  }, [enabled, path, listedPath, query])

  return { locations, refreshLocations }
}
