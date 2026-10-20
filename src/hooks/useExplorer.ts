import { startTransition, useCallback, useEffect, useRef, useState } from 'react'
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
  const [directoryReadOnly, setDirectoryReadOnly] = useState(false)
  const [virtualVolume, setVirtualVolume] = useState(false)
  const [readOnly, setReadOnly] = useState(false)
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
  const [gitPath, setGitPath] = useState('')
  const [gitLoading, setGitLoading] = useState(false)
  const gitRequest = useRef(0)
  const [connections, setConnections] = useState<RemoteConnection[]>([])
  const [renaming, setRenaming] = useState<string | null>(null)
  const [listedPath, setListedPath] = useState('')
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
      .then(async (next) => {
        const initial = native ? await call<string | null>('initial_path') : null
        const home = next.find((x) => x.name === '文稿') || next[0]
        if (home) {
          const start = initial || home.path
          setPath(start)
          setHistory([start])
          setCursor(0)
          setTabs([start])
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
          setDirectoryReadOnly(Boolean(l.readOnly))
          setVirtualVolume(Boolean(l.virtualVolume))
          setListedPath(path)
          setEntries(l.entries)
          setWarnings(l.warnings)
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
  }, [path, prefs.hidden, prefs.displayIgnore, refresh, query])
  useEffect(() => {
    if (!path || !native || virtualVolume || path.startsWith('history://')) return
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
  }, [path, reload, virtualVolume])
  const refreshGit = useCallback(async () => {
    const id = ++gitRequest.current
    if (!path || path.startsWith('remote://') || path.startsWith('usbfs://')) {
      setGit(null)
      setGitLoading(false)
      setGitPath(path)
      return
    }
    setGitLoading(true)
    try {
      const value = await call<GitStatus | null>('git_status', { path })
      if (id === gitRequest.current) setGit(value)
    } catch {
      if (id === gitRequest.current) setGit(null)
    } finally {
      if (id === gitRequest.current) {
        setGitPath(path)
        setGitLoading(false)
      }
    }
  }, [path])
  useEffect(() => {
    void refreshGit()
    return () => {
      ++gitRequest.current
    }
  }, [refresh, refreshGit])
  useEffect(() => {
    if (!path || virtualVolume || path.startsWith('remote://') || path.startsWith('history://'))
      return
    const timer = setTimeout(() => {
      void call('warm_index', {
        path,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
      }).catch(() => {})
    }, 250)
    return () => clearTimeout(timer)
  }, [path, prefs.hidden, prefs.displayIgnore, refresh, virtualVolume])
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
        startTransition(() => setEntries((v) => [...v, ...b.entries]))
        if (b.done) {
          setSearching(false)
          setSearchStats(
            `${b.elapsedMs} ms${b.truncated ? ' · 已达到 500 项上限，请缩小范围' : ''}${b.skipped ? ` · ${b.skipped} 个位置无法访问` : ''}`,
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
    void refreshConnections()
  }, [refreshConnections])
  return {
    path,
    entries: visibleEntries,
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
    git: gitPath === path ? git : null,
    gitLoading: gitLoading || (!!path && gitPath !== path),
    refreshGit,
    selected,
    setSelected,
    loading: loading || (!query && listedPath !== path && !error),
    hasListing: listedPath === path,
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
