import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { call, native, search } from '@/lib/api'
import { parent } from '@/lib/files'
import type { Entry, Listing, Location, Preferences } from '@/lib/types'
export function useExplorer(prefs: Preferences) {
  const [locations, setLocations] = useState<Location[]>([])
  const [path, setPath] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
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
  const request = useRef(0)
  const searchRequest = useRef(0)
  const reload = useCallback(() => setRefresh((v) => v + 1), [])
  const navigate = useCallback(
    (next: string) => {
      setQuery('')
      setPath(next)
      setSelected([])
      setError('')
      setHistory((h) => [...h.slice(0, cursor + 1), next])
      setCursor((c) => c + 1)
      setTabs((t) => t.map((p, i) => (i === tab ? next : p)))
    },
    [cursor, tab],
  )
  useEffect(() => {
    void call<Location[]>('locations')
      .then((l) => {
        setLocations(l)
        const home = l.find((x) => x.name === '文稿') || l[0]
        if (home) {
          setPath(home.path)
          setHistory([home.path])
          setCursor(0)
          setTabs([home.path])
        }
      })
      .catch((e) => setError(String(e)))
  }, [])
  useEffect(() => {
    if (!path || query) return
    let active = true
    const id = ++request.current
    setLoading(true)
    setError('')
    void call<Listing>('list_directory', { path, hidden: prefs.hidden })
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
      void search(path, query, prefs.hidden, prefs.respectIgnore, (b) => {
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
  }, [query, path, prefs.hidden, prefs.respectIgnore])
  const back = () => {
    if (cursor > 0) {
      setCursor(cursor - 1)
      setPath(history[cursor - 1])
      setTabs((t) => t.map((p, i) => (i === tab ? history[cursor - 1] : p)))
      setQuery('')
      setSelected([])
    }
  }
  const forward = () => {
    if (cursor < history.length - 1) {
      setCursor(cursor + 1)
      setPath(history[cursor + 1])
      setTabs((t) => t.map((p, i) => (i === tab ? history[cursor + 1] : p)))
      setQuery('')
      setSelected([])
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
  }
  const switchTab = (i: number) => {
    setTab(i)
    setPath(tabs[i])
    setHistory([tabs[i]])
    setCursor(0)
    setQuery('')
    setSelected([])
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
  }
  return {
    path,
    entries,
    locations,
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
  }
}
export type Explorer = ReturnType<typeof useExplorer>
