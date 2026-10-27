import { useEffect, useMemo, useRef, useState } from 'react'
import { getLocale } from '@/i18n'
import { call } from '@/lib/api'
import { clearDecorationIcons, syncDecorationIcons } from '@/lib/decorationIcons'
import { resolveDecorations } from '@/lib/decorations'
import type { Entry, EntryDecoration } from '@/lib/types'

/** Decorate listing metadata immediately, then merge validated plugin results.
 * A failed or stale plugin response cannot remove builtin presentation. */
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
    values: Record<string, EntryDecoration>
  }>()
  const [iconOwner] = useState(() => crypto.randomUUID())
  const request = useRef(0)
  const locale = getLocale()
  const decorations = useMemo(
    () =>
      resolveDecorations(
        entries,
        enabled && !query && snapshot?.entries === entries ? snapshot.values : {},
      ),
    [entries, snapshot, enabled, query, locale],
  )

  useEffect(() => {
    syncDecorationIcons(decorations, iconOwner)
  }, [decorations, iconOwner])
  useEffect(() => () => clearDecorationIcons(iconOwner), [iconOwner])
  useEffect(() => {
    if (!enabled || query || !path || !entries.length) return
    const generation = ++request.current
    const timer = window.setTimeout(() => {
      void call<EntryDecoration[]>('plugin_decorations', {
        directory: path,
        entries: entries.map(({ path, name, isDir, extension, mime }) => ({
          path,
          name,
          isDir,
          extension,
          mime,
        })),
      })
        .then((values) => {
          if (generation !== request.current) return
          setSnapshot({
            entries,
            values: Object.fromEntries(values.map((value) => [value.path, value])),
          })
        })
        .catch(() => {
          if (generation === request.current) setSnapshot({ entries, values: {} })
        })
    }, 120)
    return () => {
      window.clearTimeout(timer)
      ++request.current
    }
  }, [enabled, path, entries, query, refresh, locale])
  return decorations
}
