import { startTransition, useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { call, native, search } from '@/lib/api'
import { syncDecorationIcons } from '@/lib/decorationIcons'
import type {
  Entry,
  EntryDecoration,
  Listing,
  Location,
  Preferences,
  RemoteConnection,
} from '@/lib/types'
import type { Navigation } from './useNavigation'

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
  const { path, query, globalSearch, history, navigate, setSelected, setRenaming, setError } = nav
  const [locations, setLocations] = useState<Location[]>([])
  const [entries, setEntries] = useState<Entry[]>([])
  const [decorations, setDecorations] = useState<Record<string, EntryDecoration>>({})
  const [tagFilter, setTagFilterState] = useState<string | null>(null)
  const [directoryReadOnly, setDirectoryReadOnly] = useState(false)
  const [virtualVolume, setVirtualVolume] = useState(false)
  const [readOnly, setReadOnly] = useState(false)
  const [loading, setLoading] = useState(false)
  const [searching, setSearching] = useState(false)
  const [searchStats, setSearchStats] = useState('')
  const [refresh, setRefresh] = useState(0)
  const [warnings, setWarnings] = useState<string[]>([])
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [listedPath, setListedPath] = useState('')
  const request = useRef(0)
  const locationsRequest = useRef(0)
  const decorationRequest = useRef(0)
  const searchRequest = useRef(0)
  useEffect(() => {
    syncDecorationIcons(decorations)
  }, [decorations])
  const reload = useCallback(() => setRefresh((v) => v + 1), [])
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
  const refreshLocations = useCallback((force = false): Promise<Location[]> => {
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
  }, [])
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
      else if (!start && prefs.startPage === 'last') start = localStorage.getItem('fiex:lastPath')
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
        if (start !== 'recent://') localStorage.setItem('fiex:lastPath', start)
        nav.startAt(start)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [refreshLocations, enabled])
  useEffect(() => {
    if (!enabled || !path || query || path.startsWith('fiex-plugin-view://')) return
    let active = true
    const id = ++request.current
    setLoading(true)
    setError('')
    void call<Listing>('list_directory', {
      path,
      hidden: prefs.hidden,
      ignores: prefs.displayIgnore,
    })
      .then((l) => {
        if (active && id === request.current) {
          if (l.path !== path) {
            navigate(l.path)
            return
          }
          setDirectoryReadOnly(Boolean(l.readOnly))
          setVirtualVolume(Boolean(l.virtualVolume))
          setListedPath(path)
          setEntries(l.entries)
          setWarnings(l.warnings)
          // A fresh install has nothing recent yet; do not greet the user with
          // an empty list.
          if (path === 'recent://' && l.entries.length === 0 && history.length <= 1) {
            const home = locations.find((item) => item.name === '个人文件夹')?.path
            if (home) navigate(home)
          }
          const paths = new Set(l.entries.map((entry) => entry.path))
          setSelected((selected) => selected.filter((path) => paths.has(path)))
        }
      })
      .catch((e) => {
        if (active) {
          setError(String(e))
          setEntries([])
        }
      })
      .finally(() => {
        if (active) setLoading(false)
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
      setWarnings([`无法监视文件变更：${e}`]),
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
    if (!enabled) return
    const generation = ++searchRequest.current
    void call('cancel_search', { pane }).catch(() => {})
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
    const timer = setTimeout(() => {
      void search(
        pane,
        globalSearch ? 'everywhere://' : path,
        query,
        prefs.hidden,
        prefs.displayIgnore,
        (b) => {
          if (generation !== searchRequest.current) return
          startTransition(() => setEntries((v) => [...v, ...b.entries]))
          if (b.done) {
            setSearching(false)
            setSearchStats(
              `${b.elapsedMs} ms${b.truncated ? ' · 已达到 500 项上限，请缩小范围' : ''}${b.skipped ? ` · ${b.skipped} 个位置无法访问` : ''}`,
            )
          }
        },
      ).catch((e) => {
        if (generation === searchRequest.current) {
          setError(String(e))
          setSearching(false)
        }
      })
      // Search providers are additive: a provider failure never suppresses
      // local results or other providers.
      void call<Entry[]>('plugin_search', {
        query,
        root: globalSearch ? 'everywhere://' : path,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
        limit: 200,
      })
        .then((extra) => {
          if (generation !== searchRequest.current || !extra.length) return
          startTransition(() =>
            setEntries((current) => {
              const seen = new Set(current.map((entry) => entry.path))
              return [...current, ...extra.filter((entry) => !seen.has(entry.path))]
            }),
          )
        })
        .catch(() => {})
    }, 200)
    return () => {
      clearTimeout(timer)
      ++searchRequest.current
      void call('cancel_search', { pane }).catch(() => {})
    }
  }, [enabled, pane, query, globalSearch, path, prefs.hidden, prefs.displayIgnore, refresh])
  useEffect(() => {
    setTagFilterState(null)
  }, [path])
  const beginRename = useCallback(
    (target: string) => {
      if (readOnly) return
      setSelected([target])
      setRenaming(target)
    },
    [readOnly],
  )
  const cancelRename = useCallback(() => setRenaming(null), [])
  const renameTo = useCallback(
    async (target: string, name: string) => {
      if (readOnly) {
        setError('当前是只读分支或版本，不能修改文件')
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
    [readOnly],
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
      setConnections(Array.isArray(list) ? list : [])
    } catch {
      setConnections([])
    }
  }, [])
  useEffect(() => {
    if (!enabled) return
    void refreshConnections()
  }, [enabled, refreshConnections])
  useEffect(() => {
    if (!enabled || query || !path || entries.length === 0) {
      setDecorations({})
      return
    }
    const generation = ++decorationRequest.current
    const timer = window.setTimeout(() => {
      void call<EntryDecoration[]>('plugin_decorations', {
        directory: path,
        entries: entries.map((entry) => ({
          path: entry.path,
          name: entry.name,
          isDir: entry.isDir,
          extension: entry.extension,
          mime: entry.mime,
        })),
      })
        .then((values) => {
          if (generation !== decorationRequest.current) return
          setDecorations(Object.fromEntries(values.map((value) => [value.path, value])))
        })
        .catch(() => {
          if (generation === decorationRequest.current) setDecorations({})
        })
    }, 120)
    return () => {
      window.clearTimeout(timer)
      ++decorationRequest.current
    }
  }, [enabled, path, entries, query, refresh])
  return {
    entries: visibleEntries,
    decorations,
    connections,
    refreshConnections,
    tagFilter,
    setTagFilter,
    readOnly,
    directoryReadOnly,
    virtualVolume,
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
