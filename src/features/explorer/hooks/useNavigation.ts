import { useCallback, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { parent } from '@/lib/files'
import { createNavigationRequestLifecycle } from '@/features/explorer/model/navigationRequest'
import {
  closeNavigationTab,
  moveNavigationTab,
  openNavigationTab,
  selectNavigationTab,
} from '@/features/explorer/model/navigationTabs'
import {
  appendNavigationPath,
  moveNavigationCursor,
  previousNavigationPath,
} from '@/features/explorer/model/navigationHistory'

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
  const [navigationRequests] = useState(createNavigationRequestLifecycle)
  const setCurrentPath = useCallback(
    (next: string) => {
      navigationRequests.invalidate()
      setPath(next)
    },
    [navigationRequests],
  )
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
      setCurrentPath(next)
      setSelected([])
      setRenaming(null)
      setError('')
      const nextHistory = appendNavigationPath(history, cursor, next)
      setHistory(nextHistory.history)
      setCursor(nextHistory.cursor)
      setTabs((t) => t.map((p, i) => (i === tab ? next : p)))
    },
    [cursor, history, setCurrentPath, tab],
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
  const startAt = useCallback(
    (start: string) => {
      setPendingSelection(null)
      setCurrentPath(start)
      setHistory([start])
      setCursor(0)
      setTabs([start])
      setTab(0)
      tabHistory.current = [{ paths: [start], cursor: 0 }]
      setQuery('')
      setSelected([])
      setRenaming(null)
      setError('')
    },
    [setCurrentPath],
  )
  const back = () => {
    const previous = moveNavigationCursor(history, cursor, -1)
    if (previous) {
      setPendingSelection(null)
      setCursor(previous.cursor)
      setCurrentPath(previous.path)
      setTabs((t) => t.map((p, i) => (i === tab ? previous.path : p)))
      setQuery('')
      setSelected([])
      setRenaming(null)
    }
  }
  const forward = () => {
    const next = moveNavigationCursor(history, cursor, 1)
    if (next) {
      setPendingSelection(null)
      setCursor(next.cursor)
      setCurrentPath(next.path)
      setTabs((t) => t.map((p, i) => (i === tab ? next.path : p)))
      setQuery('')
      setSelected([])
      setRenaming(null)
    }
  }
  const newTab = (next = path) => {
    setPendingSelection(null)
    const opened = openNavigationTab({
      tabs,
      histories: tabHistory.current,
      active: tab,
      currentHistory: { paths: history, cursor },
      path: next,
    })
    tabHistory.current = opened.histories
    setTabs(opened.tabs)
    setTab(opened.active)
    setCurrentPath(opened.path)
    setHistory(opened.history.paths)
    setCursor(opened.history.cursor)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  const switchTab = (i: number) => {
    const selectedTab = selectNavigationTab({
      tabs,
      histories: tabHistory.current,
      active: tab,
      currentHistory: { paths: history, cursor },
      index: i,
    })
    if (!selectedTab) return
    setPendingSelection(null)
    tabHistory.current = selectedTab.histories
    setTab(selectedTab.active)
    setCurrentPath(selectedTab.path)
    setHistory(selectedTab.history.paths)
    setCursor(selectedTab.history.cursor)
    setQuery('')
    setSelected([])
    setRenaming(null)
  }
  const closeTab = (i: number, startPath = 'recent://') => {
    const closingLastTab = tabs.length === 1
    const closed = closeNavigationTab({
      tabs,
      histories: tabHistory.current,
      active: tab,
      currentHistory: { paths: history, cursor },
      index: i,
      startPath,
    })
    if (!closed) return
    setPendingSelection(null)
    // The last folder tab closes onto the configured start page in the same
    // render, so the workspace never flashes an empty tab strip.
    tabHistory.current = closed.histories
    setTabs(closed.tabs)
    setTab(closed.active)
    setCurrentPath(closed.path)
    setHistory(closed.history.paths)
    setCursor(closed.history.cursor)
    setQuery('')
    setSelected([])
    setRenaming(null)
    if (closingLastTab) setError('')
  }
  const moveTab = (from: number, to: number) => {
    const moved = moveNavigationTab({
      tabs,
      histories: tabHistory.current,
      active: tab,
      currentHistory: { paths: history, cursor },
      from,
      to,
    })
    if (!moved) return
    tabHistory.current = moved.histories
    setTabs(moved.tabs)
    setTab(moved.active)
  }
  const referencedPaths = [
    ...history,
    ...tabs,
    ...tabHistory.current.flatMap((item, index) => (index === tab ? [] : item.paths)),
  ]
  const snapshotNavigation = () => ({ path, history, cursor })
  const restoreNavigation = (state: { path: string; history: string[]; cursor: number }) => {
    setPendingSelection(null)
    setCurrentPath(state.path)
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
    previousPath: previousNavigationPath(history, cursor),
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
      const result = await navigationRequests.run(() =>
        path.startsWith('applications://')
          ? call('file_parent', { path })
          : Promise.resolve(uriParent),
      )
      if (result.status === 'stale') return
      if (result.status === 'failed') {
        setError(String(result.error))
        return
      }
      if (result.value !== path) navigate(result.value)
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
