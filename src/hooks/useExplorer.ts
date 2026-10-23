import { startTransition, useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { call, native, search } from '@/lib/api'
import { parent } from '@/lib/files'
import type {
  Entry,
  EntryDecoration,
  Listing,
  Location,
  Preferences,
  RemoteConnection,
} from '@/lib/types'
export function useExplorer(
  prefs: Preferences,
  startOverride?: string,
  enabled = true,
  pane = 'left',
) {
  const [locations, setLocations] = useState<Location[]>([])
  const [path, setPath] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const [decorations, setDecorations] = useState<Record<string, EntryDecoration>>({})
  const [tagFilter, setTagFilterState] = useState<string | null>(null)
  const [directoryReadOnly, setDirectoryReadOnly] = useState(false)
  const [virtualVolume, setVirtualVolume] = useState(false)
  const [readOnly, setReadOnly] = useState(false)
  const [selected, setSelected] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [query, setQuery] = useState('')
  const [globalSearch, setGlobalSearch] = useState(false)
  const [searching, setSearching] = useState(false)
  const [searchStats, setSearchStats] = useState('')
  const [history, setHistory] = useState<string[]>([])
  const [cursor, setCursor] = useState(-1)
  const [refresh, setRefresh] = useState(0)
  const [tabs, setTabs] = useState<string[]>([])
  const [tab, setTab] = useState(0)
  const tabHistory = useRef<{ paths: string[]; cursor: number }[]>([])
  const [warnings, setWarnings] = useState<string[]>([])
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [renaming, setRenaming] = useState<string | null>(null)
  const [listedPath, setListedPath] = useState('')
  const request = useRef(0)
  const decorationRequest = useRef(0)
  const searchRequest = useRef(0)
  const reload = useCallback(() => setRefresh((v) => v + 1), [])
  const navigate = useCallback(
    (next: string) => {
      setQuery('')
      setPath(next)
      setSelected([])
      setRenaming(null)
      setError('')
      setHistory((h) => [...h.slice(0, cursor + 1), next])
      setCursor((c) => c + 1)
      setTabs((t) => t.map((p, i) => (i === tab ? next : p)))
    },
    [cursor, tab],
  )
  const locationsKey = useRef('')
  const locationsPending = useRef<Promise<Location[]> | null>(null)
  const refreshLocations = useCallback((): Promise<Location[]> => {
    if (locationsPending.current) return locationsPending.current
    const pending = call<Location[]>('locations')
      .then((next) => {
        const key = JSON.stringify(next)
        if (key !== locationsKey.current) {
          locationsKey.current = key
          setLocations(next)
        }
        return next
      })
      .finally(() => {
        locationsPending.current = null
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
        setPath(start)
        setHistory([start])
        setCursor(0)
        setTabs([start])
        tabHistory.current = [{ paths: [start], cursor: 0 }]
      }
    })()
    return () => {
      cancelled = true
    }
  }, [refreshLocations, enabled])
  useEffect(() => {
    if (!enabled || !path || query) return
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
    if (!enabled || !path || !native || virtualVolume || path.startsWith('recent://')) return
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
  const back = () => {
    if (cursor > 0) {
      setCursor(cursor - 1)
      setPath(history[cursor - 1])
      setTabs((t) => t.map((p, i) => (i === tab ? history[cursor - 1] : p)))
      setQuery('')
      setSelected([])
      setRenaming(null)
    }
  }
  const forward = () => {
    if (cursor < history.length - 1) {
      setCursor(cursor + 1)
      setPath(history[cursor + 1])
      setTabs((t) => t.map((p, i) => (i === tab ? history[cursor + 1] : p)))
      setQuery('')
      setSelected([])
      setRenaming(null)
    }
  }
  const newTab = (next = path) => {
    tabHistory.current[tab] = { paths: history, cursor }
    tabHistory.current.push({ paths: [next], cursor: 0 })
    setTabs((t) => [...t, next])
    setTab(tabs.length)
    setPath(next)
    setHistory([next])
    setCursor(0)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  const switchTab = (i: number) => {
    if (!tabs[i] || i === tab) return
    tabHistory.current[tab] = { paths: history, cursor }
    const saved = tabHistory.current[i] || { paths: [tabs[i]], cursor: 0 }
    setTab(i)
    setPath(tabs[i])
    setHistory(saved.paths)
    setCursor(saved.cursor)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  const closeTab = (i: number) => {
    if (tabs.length === 1) return
    tabHistory.current[tab] = { paths: history, cursor }
    tabHistory.current.splice(i, 1)
    const next = tabs.filter((_, n) => n !== i)
    const n = i < tab ? tab - 1 : Math.min(tab, next.length - 1)
    const saved = tabHistory.current[n] || { paths: [next[n]], cursor: 0 }
    setTabs(next)
    setTab(n)
    setPath(next[n])
    setHistory(saved.paths)
    setCursor(saved.cursor)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  const moveTab = (from: number, to: number) => {
    if (from === to || !tabs[from] || !tabs[to]) return
    tabHistory.current[tab] = { paths: history, cursor }
    const next = [...tabs]
    next.splice(to, 0, next.splice(from, 1)[0])
    const histories = tabHistory.current
    histories.splice(to, 0, histories.splice(from, 1)[0])
    setTabs(next)
    setTab(
      tab === from
        ? to
        : from < tab && to >= tab
          ? tab - 1
          : from > tab && to <= tab
            ? tab + 1
            : tab,
    )
  }
  const snapshotNavigation = () => ({ path, history, cursor })
  const restoreNavigation = (state: { path: string; history: string[]; cursor: number }) => {
    setPath(state.path)
    setHistory(state.history)
    setCursor(state.cursor)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
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
    path,
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
    selected,
    setSelected,
    loading: loading || (!query && listedPath !== path && !error),
    hasListing: listedPath === path,
    error,
    setError,
    warnings,
    query,
    setQuery,
    globalSearch,
    setGlobalSearch,
    searching,
    searchStats,
    navigate,
    reload,
    back,
    forward,
    canBack: cursor > 0,
    canForward: cursor < history.length - 1,
    up: () => {
      if (path !== 'recent://') navigate(parent(path))
    },
    tabs,
    tab,
    newTab,
    switchTab,
    closeTab,
    moveTab,
    snapshotNavigation,
    restoreNavigation,
    renaming,
    beginRename,
    cancelRename,
    renameTo,
  }
}
export type Explorer = ReturnType<typeof useExplorer>
