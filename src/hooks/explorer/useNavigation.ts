import { useCallback, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { parent } from '@/lib/files'

export interface PendingSelection {
  directory: string
  target: string
  generation: number
}

/** Path, history, tab state and the transient per-pane view state every
 *  navigation resets: selection, rename target and the last error. The listing
 *  itself is loaded by useListing, which reads these values. */
export function useNavigation(initialTabs?: { paths: string[]; active: number }) {
  const initialIndex = initialTabs?.paths.length
    ? Math.min(Math.max(0, initialTabs.active), initialTabs.paths.length - 1)
    : -1
  const initialPath = initialIndex >= 0 ? initialTabs!.paths[initialIndex] : ''
  const [path, setPath] = useState(initialPath)
  const [selected, setSelected] = useState<string[]>([])
  const [pendingSelection, setPendingSelection] = useState<PendingSelection | null>(null)
  const pendingSelectionGeneration = useRef(0)
  const [renaming, setRenaming] = useState<string | null>(null)
  const [error, setError] = useState('')
  const [query, setQuery] = useState('')
  const [globalSearch, setGlobalSearch] = useState(false)
  // The search mode is a pane preference, not query state: switching modes
  // cancels the old session and re-runs the query with the other capability.
  const [searchMode, setSearchModeState] = useState<'name' | 'content'>(() =>
    localStorage.getItem('fiex:search-mode') === 'content' ? 'content' : 'name',
  )
  const setSearchMode = useCallback((mode: 'name' | 'content') => {
    setSearchModeState(mode)
    localStorage.setItem('fiex:search-mode', mode)
  }, [])
  const [history, setHistory] = useState<string[]>(initialPath ? [initialPath] : [])
  const [cursor, setCursor] = useState(initialPath ? 0 : -1)
  const [tabs, setTabs] = useState<string[]>(initialTabs?.paths || [])
  const [tab, setTab] = useState(initialIndex >= 0 ? initialIndex : 0)
  const tabHistory = useRef<{ paths: string[]; cursor: number }[]>(
    initialTabs?.paths.map((value) => ({ paths: [value], cursor: 0 })) || [],
  )
  const setLocation = useCallback(
    (next: string, target?: string) => {
      setPendingSelection(
        target
          ? { directory: next, target, generation: ++pendingSelectionGeneration.current }
          : null,
      )
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
  const navigate = useCallback((next: string) => setLocation(next), [setLocation])
  const navigateTo = useCallback(
    (directory: string, target: string) => setLocation(directory, target),
    [setLocation],
  )
  const clearPendingSelection = useCallback((generation: number) => {
    setPendingSelection((current) => (current?.generation === generation ? null : current))
  }, [])
  // Start folder resolution is asynchronous; the first path arrives here.
  const startAt = (start: string) => {
    setPendingSelection(null)
    setPath(start)
    setHistory([start])
    setCursor(0)
    setTabs([start])
    setTab(0)
    tabHistory.current = [{ paths: [start], cursor: 0 }]
    setQuery('')
    setSelected([])
    setRenaming(null)
    setError('')
  }
  const back = () => {
    if (cursor > 0) {
      setPendingSelection(null)
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
      setPendingSelection(null)
      setCursor(cursor + 1)
      setPath(history[cursor + 1])
      setTabs((t) => t.map((p, i) => (i === tab ? history[cursor + 1] : p)))
      setQuery('')
      setSelected([])
      setRenaming(null)
    }
  }
  const newTab = (next = path) => {
    setPendingSelection(null)
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
    setPendingSelection(null)
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
  const closeTab = (i: number, startPath = 'recent://') => {
    if (!tabs[i]) return
    setPendingSelection(null)
    // The last folder tab closes onto the configured start page in the same
    // render, so the workspace never flashes an empty tab strip.
    if (tabs.length === 1) {
      startAt(startPath)
      return
    }
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
  const referencedPaths = [
    ...history,
    ...tabs,
    ...tabHistory.current.flatMap((item, index) => (index === tab ? [] : item.paths)),
  ]
  const snapshotNavigation = () => ({ path, history, cursor })
  const restoreNavigation = (state: { path: string; history: string[]; cursor: number }) => {
    setPendingSelection(null)
    setPath(state.path)
    setTabs((paths) => paths.map((path, index) => (index === tab ? state.path : path)))
    tabHistory.current[tab] = { paths: state.history, cursor: state.cursor }
    setHistory(state.history)
    setCursor(state.cursor)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  return {
    path,
    referencedPaths,
    selected,
    setSelected,
    pendingSelection,
    clearPendingSelection,
    renaming,
    setRenaming,
    error,
    setError,
    query,
    setQuery,
    globalSearch,
    setGlobalSearch,
    searchMode,
    setSearchMode,
    history,
    navigate,
    navigateTo,
    startAt,
    back,
    forward,
    canBack: cursor > 0,
    canForward: cursor < history.length - 1,
    up: async () => {
      const uriParent = parent(path)
      if (uriParent === path) return
      try {
        const upPath = path.startsWith('applications://')
          ? await call('file_parent', { path })
          : uriParent
        if (upPath !== path) navigate(upPath)
      } catch (cause) {
        setError(String(cause))
      }
    },
    tabs,
    tab,
    newTab,
    switchTab,
    closeTab,
    moveTab,
    snapshotNavigation,
    restoreNavigation,
  }
}
export type Navigation = ReturnType<typeof useNavigation>
