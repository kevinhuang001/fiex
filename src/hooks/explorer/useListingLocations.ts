import { useCallback, useEffect, useRef, useState } from 'react'
import { call, native } from '@/lib/api'
import { getLocale } from '@/i18n'
import type { Location, Preferences } from '@/lib/types'
import type { Navigation } from '@/features/explorer/model/useNavigation'

export function resolveInitialPath({
  startOverride,
  launched,
  startPage,
  lastPath,
  initialPath,
  locations,
}: {
  startOverride?: string
  launched?: string | null
  startPage: Preferences['startPage']
  lastPath?: string | null
  initialPath?: string | null
  locations: Location[]
}): string | null {
  let start = startOverride || launched || null
  if (!start && startPage === 'recent') start = 'recent://'
  else if (!start && startPage === 'last') start = lastPath || null
  if (start) return start

  const pick = (name: string) => locations.find((item) => item.name === name)?.path
  return startPage === 'home'
    ? (pick('个人文件夹') ?? initialPath ?? null)
    : (pick('文稿') ?? initialPath ?? locations[0]?.path ?? null)
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
  const [locations, setLocations] = useState<Location[]>([])
  const locationsRequest = useRef(0)
  const locationsKey = useRef('')
  const locationsPending = useRef<Promise<Location[]> | null>(null)
  const refreshLocations = useCallback(
    (force = false): Promise<Location[]> => {
      if (locationsPending.current && !force) return locationsPending.current
      const request = ++locationsRequest.current
      const pending = call('locations')
        .then((next) => {
          if (request !== locationsRequest.current) return next
          const key = JSON.stringify(next)
          if (key !== locationsKey.current) {
            locationsKey.current = key
            setLocations(next)
          }
          return next
        })
        .finally(() => {
          if (request === locationsRequest.current) locationsPending.current = null
        })
      locationsPending.current = pending
      return pending
    },
    [getLocale()],
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
    void (async () => {
      const launched = native ? await call('launch_path').catch(() => null) : null
      if (cancelled) return
      let lastPath: string | null = null
      if (!startOverride && !launched && prefs.startPage === 'last') {
        lastPath = await call('state_get', {
          namespace: 'explorer',
          key: 'lastPath',
        }).catch(() => null)
        if (!lastPath) lastPath = localStorage.getItem('fiex:lastPath')
      }
      if (cancelled) return
      const needsLocation = !startOverride && !launched && prefs.startPage !== 'recent' && !lastPath
      const initial = needsLocation && native ? await call('initial_path').catch(() => null) : null
      if (cancelled) return
      const available = needsLocation ? await call('locations').catch(() => [] as Location[]) : []
      if (cancelled) return
      const start = resolveInitialPath({
        startOverride,
        launched,
        startPage: prefs.startPage,
        lastPath,
        initialPath: initial,
        locations: available,
      })
      if (start) nav.startAt(start)
    })()
    return () => {
      cancelled = true
    }
  }, [refreshLocations, enabled, prefs.startPage, startOverride])

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
