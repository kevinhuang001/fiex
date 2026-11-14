import { useEffect, useRef, useState } from 'react'
import type { Entry, EntryDecoration } from '@/lib/types'

export interface EntryDecorationServices {
  getLocale: () => string
  load: (directory: string, entries: Entry[], includePlugins: boolean) => Promise<EntryDecoration[]>
  cache: (entries: Entry[], decorations: EntryDecoration[]) => void
  publishIcons: (entries: Entry[]) => void
  cached: (entries: Entry[]) => Record<string, EntryDecoration>
}

/** Receive the unified Core decoration DTO. Search requests builtin providers
 * only; stale responses never paint onto a different listing. */
export function useEntryDecorations({
  entries,
  path,
  enabled,
  query,
  refresh,
  services,
}: {
  entries: Entry[]
  path: string
  enabled: boolean
  query: string
  refresh: number
  services: EntryDecorationServices
}) {
  const [snapshot, setSnapshot] = useState<{
    entries: Entry[]
    key: string
    values: Record<string, EntryDecoration>
  }>()
  const request = useRef(0)
  const locale = services.getLocale()
  const key = JSON.stringify([path, Boolean(query), locale])
  const decorations =
    snapshot?.entries === entries && snapshot.key === key
      ? snapshot.values
      : services.cached(entries)

  useEffect(() => {
    if (!enabled || !path || !entries.length) return
    const requestRef = request
    const generation = ++requestRef.current
    const timer = window.setTimeout(() => {
      void services
        .load(path, entries, !query)
        .then((values) => {
          if (generation !== requestRef.current) return
          services.cache(entries, values)
          services.publishIcons(entries)
          setSnapshot({
            entries,
            key,
            values: services.cached(entries),
          })
        })
        .catch(() => {
          // Keep the last complete presentation if revalidation fails.
          if (generation === requestRef.current)
            setSnapshot({ entries, key, values: services.cached(entries) })
        })
    }, 0)
    return () => {
      window.clearTimeout(timer)
      ++requestRef.current
    }
  }, [enabled, path, entries, query, refresh, locale, key, services])
  return decorations
}
