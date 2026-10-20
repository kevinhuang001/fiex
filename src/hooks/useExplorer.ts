import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { call, native, search } from '@/lib/api'
import { parent } from '@/lib/files'
import type {
  Entry,
  GitStatus,
  Listing,
  Location,
  Preferences,
  RemoteConnection,
} from '@/lib/types'
export function useExplorer(prefs: Preferences) {
  const [locations, setLocations] = useState<Location[]>([])
  const [path, setPath] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const [tagFilter, setTagFilterState] = useState<string | null>(null)
  const [selected, setSelected] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [query, setQuery] = useState('')
  const [searching, setSearching] = useState(false)
  const [searchStats, setSearchStats] = useState('')
  const [history, setHistory] = useState<string[]>([])
  const [cursor, setCursor] = useState(-1)
  const [refresh, setRefresh] = useState(0)
  const [tabs, setTabs] = useState<string[]>([])
  const [tab, setTab] = useState(0)
  const [warnings, setWarnings] = useState<string[]>([])
  const [git, setGit] = useState<GitStatus | null>(null)
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [renaming, setRenaming] = useState<string | null>(null)
  const request = useRef(0)
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
  const refreshLocations = useCallback(async () => {
    const next = await call<Location[]>('locations')
    setLocations(next)
    return next
  }, [])
  useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshLocations().catch(() => {})
    }, 15000)
    return () => window.clearInterval(timer)
  }, [refreshLocations])
  useEffect(() => {
    void refreshLocations()
      .then((next) => {
        const home = next.find((x) => x.name === '文稿') || next[0]
        if (home) {
          setPath(home.path)
          setHistory([home.path])
          setCursor(0)
          setTabs([home.path])
        }
      })
      .catch((e) => setError(String(e)))
  }, [refreshLocations])
  useEffect(() => {
    if (!path || query) return
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
          setEntries(l.entries)
          setWarnings(l.warnings)
          setSelected((s) => s.filter((p) => l.entries.some((e) => e.path === p)))
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
  }, [path, prefs.hidden, refresh, query])
  useEffect(() => {
    if (!path || !native) return
    let cleanup: (() => void) | undefined
    let active = true
    let timer: ReturnType<typeof setTimeout>
    void call('watch_directory', { path }).catch((e) => setWarnings([`无法监视文件变更：${e}`]))
    void listen('directory-changed', () => {
      clearTimeout(timer)
      timer = setTimeout(reload, 150)
    }).then((fn) => {
      if (active) cleanup = fn
      else fn()
    })
    return () => {
      active = false
      clearTimeout(timer)
      cleanup?.()
    }
  }, [path, reload])
  useEffect(() => {
    if (!path) return
    if (path.startsWith('remote://')) {
      setGit(null)
      return
    }
    let active = true
    void call<GitStatus | null>('git_status', { path })
      .then((value) => {
        if (active) setGit(value)
      })
      .catch(() => {
        if (active) setGit(null)
      })
    return () => {
      active = false
    }
  }, [path, refresh])
  useEffect(() => {
    if (!path || path.startsWith('remote://')) return
    const timer = setTimeout(() => {
      void call('warm_index', {
        path,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
      }).catch(() => {})
    }, 250)
    return () => clearTimeout(timer)
  }, [path, prefs.hidden, prefs.displayIgnore, refresh])
  useEffect(() => {
    const generation = ++searchRequest.current
    void call('cancel_search').catch(() => {})
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
      void search(path, query, prefs.hidden, prefs.displayIgnore, (b) => {
        if (generation !== searchRequest.current) return
        setEntries((v) => [...v, ...b.entries])
        if (b.done) {
          setSearching(false)
          setSearchStats(
            `${b.elapsedMs} ms${b.truncated ? ' · 已达到 10,000 项上限，请缩小范围' : ''}${b.skipped ? ` · ${b.skipped} 个位置无法访问` : ''}`,
          )
        }
      }).catch((e) => {
        if (generation === searchRequest.current) {
          setError(String(e))
          setSearching(false)
        }
      })
    }, 200)
    return () => {
      clearTimeout(timer)
      ++searchRequest.current
      void call('cancel_search').catch(() => {})
    }
  }, [query, path, prefs.hidden, prefs.displayIgnore, refresh])
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
    setTab(i)
    setPath(tabs[i])
    setHistory([tabs[i]])
    setCursor(0)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  const closeTab = (i: number) => {
    if (tabs.length === 1) return
    const next = tabs.filter((_, n) => n !== i)
    const n = i < tab ? tab - 1 : Math.min(tab, next.length - 1)
    setTabs(next)
    setTab(n)
    setPath(next[n])
    setHistory([next[n]])
    setCursor(0)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  const beginRename = useCallback((target: string) => {
    setSelected([target])
    setRenaming(target)
  }, [])
  const cancelRename = useCallback(() => setRenaming(null), [])
  const renameTo = useCallback(async (target: string, name: string) => {
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
  }, [])
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
    void refreshConnections()
  }, [refreshConnections])
  return {
    path,
    entries: visibleEntries,
    connections,
    refreshConnections,
    tagFilter,
    setTagFilter,
    locations,
    refreshLocations,
    git,
    selected,
    setSelected,
    loading,
    error,
    setError,
    warnings,
    query,
    setQuery,
    searching,
    searchStats,
    navigate,
    reload,
    back,
    forward,
    canBack: cursor > 0,
    canForward: cursor < history.length - 1,
    up: () => navigate(parent(path)),
    tabs,
    tab,
    newTab,
    switchTab,
    closeTab,
    renaming,
    beginRename,
    cancelRename,
    renameTo,
  }
}
export type Explorer = ReturnType<typeof useExplorer>
