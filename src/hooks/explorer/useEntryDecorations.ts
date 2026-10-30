import { useEffect, useRef, useState } from 'react'
import { getLocale } from '@/i18n'
import { call } from '@/lib/api'
import { publishPresentationIcons } from '@/lib/decorationIcons'
import { cachedPresentations, cachePresentations } from '@/lib/entryPresentation'
import type { Entry, EntryDecoration } from '@/lib/types'

/** Receive the unified Core decoration DTO. Search requests builtin providers
 * only; stale responses never paint onto a different listing. */
export function useEntryDecorations({
  entries,
  path,
  enabled,
  query,
  refresh,
}: {
  entries: Entry[]
  path: string
  enabled: boolean
  query: string
  refresh: number
}) {
  const [snapshot, setSnapshot] = useState<{
    entries: Entry[]
    key: string
    values: Record<string, EntryDecoration>
  }>()
  const request = useRef(0)
  const locale = getLocale()
  const key = JSON.stringify([path, Boolean(query), locale])
  const decorations =
    snapshot?.entries === entries && snapshot.key === key
      ? snapshot.values
      : cachedPresentations(entries)

  useEffect(() => {
    if (!enabled || !path || !entries.length) return
    const generation = ++request.current
    const timer = window.setTimeout(() => {
      void call<EntryDecoration[]>('plugin_decorations', {
        directory: path,
        entries,
        includePlugins: !query,
      })
        .then((values) => {
          if (generation !== request.current) return
          cachePresentations(entries, values)
          publishPresentationIcons(entries)
          setSnapshot({
            entries,
            key,
            values: Object.fromEntries(values.map((value) => [value.path, value])),
          })
        })
        .catch(() => {
          // Keep the last complete presentation if revalidation fails.
          if (generation === request.current)
            setSnapshot({ entries, key, values: cachedPresentations(entries) })
        })
    }, 0)
    return () => {
      window.clearTimeout(timer)
      ++request.current
    }
  }, [enabled, path, entries, query, refresh, locale])
  return decorations
}
