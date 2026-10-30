import { useEntryDecorations } from './useEntryDecorations'
import { cachePresentations } from '@/lib/entryPresentation'
import { publishPresentationIcons } from '@/lib/decorationIcons'
import { t, getLocale } from '@/i18n'
import { startTransition, useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { call, native, search } from '@/lib/api'
import { openDirectoryStream } from '@/lib/listing'
import type { Entry, EntryDecoration, Location, Preferences, RemoteConnection } from '@/lib/types'
import type { Navigation } from './useNavigation'

/** True for every path owned by the three Core virtual views, including their
 *  subdirectories. A local SearchProvider cannot enumerate these. */
function virtualViewPath(value: string, trashRoot: string | null): boolean {
  if (
    value === 'recent://' ||
    value.startsWith('applications://') ||
    value === 'recyclebin://' ||
    value === 'trash://'
  ) {
    return true
  }
  if (!trashRoot) return false
  const root = trashRoot.replace(/[/\\]+$/, '')
  return value === root || value.startsWith(`${root}/`) || value.startsWith(`${root}\\`)
}

/** Lightweight identity of a listing: order plus the fields that visibly
 *  change. Used to skip a redundant state replacement after the cache paint. */
function listingKey(entries: Entry[]): string {
  return JSON.stringify([...entries].sort((a, b) => a.path.localeCompare(b.path)))
}

/** Everything a pane lists: entries, decorations, locations, connections,
 *  search results and the read-only/virtual-volume flags. Path, history and tab
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
    globalSearch,
    searchMode,
    history,
    navigate,
    setSelected,
    setRenaming,
    setError,
  } = nav
  const [locations, setLocations] = useState<Location[]>([])
  const [entries, setEntries] = useState<Entry[]>([])
  const [tagFilter, setTagFilterState] = useState<string | null>(null)
  const [directoryReadOnly, setDirectoryReadOnly] = useState(false)
  const [directoryCanCreate, setDirectoryCanCreate] = useState(true)
  const [virtualVolume, setVirtualVolume] = useState(false)
  const [defaultView, setDefaultView] = useState<'list' | 'icons' | 'gallery' | 'columns' | null>(
    null,
  )
  const [readOnly, setReadOnly] = useState(false)
  const [loading, setLoading] = useState(false)
  const [searching, setSearching] = useState(false)
  const [searchStats, setSearchStats] = useState('')
  const [refresh, setRefresh] = useState(0)
  const [warnings, setWarnings] = useState<string[]>([])
  const [directoryDecorations, setDirectoryDecorations] = useState<Record<string, EntryDecoration>>(
    {},
  )
  const searchDecorations = useEntryDecorations({
    entries,
    path,
    enabled: enabled && (Boolean(query) || !native),
    query,
    refresh,
  })
  const decorations = query || !native ? searchDecorations : directoryDecorations
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [listedPath, setListedPath] = useState('')
  const request = useRef(0)
  const locationsRequest = useRef(0)
  const searchRequest = useRef(0)
  const cancelQueue = useRef<Promise<unknown>>(Promise.resolve())
  const trashRoot = useRef<string | null>(null)
  const reload = useCallback(() => setRefresh((v) => v + 1), [getLocale()])
  useEffect(() => {
    if (!native || !enabled) return
    let active = true
    void call<string | null>('trash_path')
      .then((root) => {
        if (active) trashRoot.current = root ?? null
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [enabled])
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
  const locationsKey = useRef('')
  const locationsPending = useRef<Promise<Location[]> | null>(null)
  const refreshLocations = useCallback(
    (force = false): Promise<Location[]> => {
      if (locationsPending.current && !force) return locationsPending.current
      const request = ++locationsRequest.current
      const pending = call<Location[]>('locations')
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
    // The start folder must not wait for the disk survey: resolve it first so
    // the first listing is requested immediately, then fill the sidebar.
    void refreshLocations().catch(() => {})
    void (async () => {
      // A folder passed on the command line wins over the start page.
      const launched = native && (await call<string | null>('launch_path').catch(() => null))
      if (cancelled) return
      let start: string | null = startOverride || launched || null
      if (!start && prefs.startPage === 'recent') start = 'recent://'
      else if (!start && prefs.startPage === 'last') {
        start = await call<string | null>('state_get', {
          namespace: 'explorer',
          key: 'lastPath',
        }).catch(() => null)
        if (!start) start = localStorage.getItem('fiex:lastPath')
      }
      if (!start) {
        const initial = native ? await call<string | null>('initial_path').catch(() => null) : null
        if (cancelled) return
        const locations = await call<Location[]>('locations').catch(() => [] as Location[])
        if (cancelled) return
        const pick = (name: string) => locations.find((item) => item.name === name)?.path
        start =
          prefs.startPage === 'home'
            ? (pick('个人文件夹') ?? initial)
            : (pick('文稿') ?? initial ?? locations[0]?.path ?? null)
      }
      if (start) {
        nav.startAt(start)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [refreshLocations, enabled])
  useEffect(() => {
    // Persist only successfully opened locations, not incomplete navigation.
    if (!enabled || !path || query || listedPath !== path) return
    void call('state_put', {
      namespace: 'explorer',
      key: 'lastPath',
      value: path,
    })
      .then(() => localStorage.removeItem('fiex:lastPath'))
      .catch(() => localStorage.setItem('fiex:lastPath', path))
  }, [enabled, path, listedPath, query])

  useEffect(() => {
    if (!enabled || !path || query || path.startsWith('fiex-plugin-view://')) return
    let active = true
    const id = ++request.current
    let paintedKey: string | null = null
    setLoading(true)
    setError('')
    void openDirectoryStream(path, prefs.hidden, prefs.displayIgnore, (snapshot) => {
      if (!active || id !== request.current) return
      const l = snapshot.listing
      if (l.path !== path) {
        navigate(l.path)
        return
      }
      const nextKey = listingKey(l.entries)
      if (snapshot.decorations.length) {
        cachePresentations(l.entries, snapshot.decorations)
        publishPresentationIcons(l.entries)
      }
      const presentation = Object.fromEntries(
        snapshot.decorations.map((value) => [value.path, value]),
      )
      setDirectoryDecorations((previous) =>
        JSON.stringify(previous) === JSON.stringify(presentation) ? previous : presentation,
      )
      setDirectoryReadOnly(Boolean(l.readOnly))
      setDirectoryCanCreate(l.canCreate !== false && !l.readOnly)
      setVirtualVolume(Boolean(l.virtualVolume))
      setDefaultView(l.defaultView || null)
      setWarnings((previous) =>
        previous.length === l.warnings.length &&
        previous.every((warning, index) => warning === l.warnings[index])
          ? previous
          : l.warnings,
      )
      if (paintedKey !== nextKey) {
        paintedKey = nextKey
        setEntries(l.entries)
        const paths = new Set(l.entries.map((entry) => entry.path))
        setSelected((selected) => selected.filter((p) => paths.has(p)))
      }
      setListedPath(path)
      setLoading(false)
      if (
        snapshot.complete &&
        path === 'recent://' &&
        l.entries.length === 0 &&
        history.length <= 1
      ) {
        const home = locations.find((item) => item.name === '个人文件夹')?.path
        if (home) navigate(home)
      }
    })
      .catch((error) => {
        if (!active || id !== request.current) return
        if (paintedKey === null) {
          setEntries([])
          setDirectoryDecorations({})
          setError(String(error))
        } else {
          // An offline refresh must not remove a correctly authorized snapshot.
          setWarnings([String(error)])
        }
      })
      .finally(() => {
        if (active && id === request.current) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [enabled, path, prefs.hidden, prefs.displayIgnore, refresh, query])
  useEffect(() => {
    if (
      !enabled ||
      !path ||
      !native ||
      virtualVolume ||
      path.startsWith('recent://') ||
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
    if (!enabled || !path) return
    const generation = ++searchRequest.current
    const cancel = () => {
      cancelQueue.current = cancelQueue.current
        .then(() => call('cancel_search', { pane }))
        .catch(() => {})
      return cancelQueue.current
    }
    const cancelled = cancel()
    if (!query.trim()) {
      setSearching(false)
      setSearchStats('')
      return
    }
    setSearching(true)
    setLoading(false)
    setEntries([])
    setSelected([])
    setError('')
    const root = globalSearch ? 'everywhere://' : path
    // A Core-owned virtual root only has an in-process entry set; local search
    // providers cannot enumerate it, so the engine stays builtin.
    const engine = virtualViewPath(root, trashRoot.current) ? 'builtin' : 'auto'
    const timer = setTimeout(() => {
      void cancelled
        .then(() => {
          if (generation !== searchRequest.current) return
          return search(
            pane,
            root,
            query,
            prefs.hidden,
            prefs.displayIgnore,
            searchMode,
            engine,
            (b) => {
              if (generation !== searchRequest.current) return
              if (b.replace) {
                // Virtual-folder results are complete snapshots.
                startTransition(() =>
                  setEntries((previous) =>
                    listingKey(previous) === listingKey(b.entries) ? previous : b.entries,
                  ),
                )
              } else if (b.entries.length) {
                startTransition(() =>
                  setEntries((previous) => {
                    const merged = new Map(previous.map((entry) => [entry.path, entry]))
                    let changed = false
                    for (const entry of b.entries) {
                      if (JSON.stringify(merged.get(entry.path)) !== JSON.stringify(entry)) {
                        merged.set(entry.path, entry)
                        changed = true
                      }
                    }
                    return changed ? [...merged.values()] : previous
                  }),
                )
              }
              if (b.done) {
                setSearching(false)
                const warning = b.warning ? `${b.warning} · ` : ''
                setSearchStats(
                  `${warning}${b.elapsedMs} ms${b.truncated ? t(' · 已达到 500 项上限，请缩小范围') : ''}${b.skipped ? t(' · {0} 个位置无法访问', b.skipped) : ''}`,
                )
              }
            },
          )
        })
        .catch((e) => {
          if (generation === searchRequest.current) {
            setError(String(e))
            setSearching(false)
          }
        })
    }, 200)
    return () => {
      clearTimeout(timer)
      ++searchRequest.current
      void cancel()
    }
  }, [
    enabled,
    pane,
    query,
    globalSearch,
    path,
    searchMode,
    prefs.hidden,
    prefs.displayIgnore,
    refresh,
  ])
  useEffect(() => {
    setTagFilterState(null)
  }, [path])
  const beginRename = useCallback(
    (target: string) => {
      if (readOnly) return
      setSelected([target])
      setRenaming(target)
    },
    [readOnly, getLocale()],
  )
  const cancelRename = useCallback(() => setRenaming(null), [getLocale()])
  const renameTo = useCallback(
    async (target: string, name: string) => {
      if (readOnly) {
        setError(t('当前是只读分支或版本，不能修改文件'))
        return false
      }
      setError('')
      try {
        const next = await call<string>('rename_entry', { path: target, name })
        setRenaming(null)
        setSelected([next])
        setRefresh((v) => v + 1)
        return true
      } catch (e) {
        setError(String(e))
        return false
      }
    },
    [readOnly, getLocale()],
  )
  const visibleEntries = tagFilter
    ? entries.filter((entry) => entry.tags.some((tag) => tag.name === tagFilter))
    : entries
  const setTagFilter = (tag: string | null) => {
    setTagFilterState(tag)
    setSelected([])
  }
  const refreshConnections = useCallback(async () => {
    try {
      const list = await call<RemoteConnection[]>('remote_connections')
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
    readOnly,
    directoryReadOnly,
    directoryCanCreate,
    virtualVolume,
    defaultView,
    setReadOnly,
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
