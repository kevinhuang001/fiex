import { useRef, useState } from 'react'
import type { Preferences } from '@/lib/types'
import { useExplorer, type Explorer } from './useExplorer'

type Side = 'left' | 'right'
type TabPane = { right: ReturnType<Explorer['snapshotNavigation']> | null; active: Side }

/** A tab owns one or two panes. Only the visible tab mounts a second explorer. */
export function usePaneLayout(prefs: Preferences, left: Explorer) {
  const [split, setSplit] = useState(false)
  const [active, setActive] = useState<Side>('left')
  const [paneWidth, setPaneWidth] = useState(46)
  const right = useExplorer(prefs, left.path, split, 'right')
  const x = split && active === 'right' ? right : left
  const tabs = useRef<TabPane[]>([])

  const save = () => {
    tabs.current[left.tab] = { right: split ? right.snapshotNavigation() : null, active }
  }
  const restore = (index: number) => {
    const pane = tabs.current[index]
    setSplit(Boolean(pane?.right))
    setActive(pane?.right ? pane.active : 'left')
    if (pane?.right) right.restoreNavigation(pane.right)
  }
  const switchWorkspaceTab = (index: number) => {
    if (index === left.tab || !left.tabs[index]) return
    save()
    restore(index)
    left.switchTab(index)
  }
  const newWorkspaceTab = () => {
    save()
    left.newTab(x.path)
    setSplit(false)
    setActive('left')
    tabs.current.push({ right: null, active: 'left' })
  }
  const closeWorkspaceTab = (index: number) => {
    if (left.tabs.length === 1) return
    save()
    tabs.current.splice(index, 1)
    const next = index < left.tab ? left.tab - 1 : Math.min(left.tab, left.tabs.length - 2)
    restore(next)
    left.closeTab(index)
  }
  const moveWorkspaceTab = (from: number, to: number) => {
    if (from === to) return
    save()
    tabs.current.splice(to, 0, tabs.current.splice(from, 1)[0])
    left.moveTab(from, to)
  }
  const openPane = () => {
    if (!right.path) right.navigate(left.path)
    setSplit(true)
  }
  const closePane = (side: Side = active) => {
    if (!split) return
    if (side === 'left' && right.path) left.navigate(right.path)
    tabs.current[left.tab] = { right: null, active: 'left' }
    setSplit(false)
    setActive('left')
  }
  const panePaths = left.tabs.map((_, index) =>
    index === left.tab ? (split ? right.path : null) : (tabs.current[index]?.right?.path ?? null),
  )
  const paneActives = left.tabs.map((_, index) =>
    index === left.tab ? active : (tabs.current[index]?.active ?? 'left'),
  )
  return {
    right,
    viewReferences: [
      ...left.referencedPaths,
      ...(split ? right.referencedPaths : []),
      ...tabs.current.flatMap((pane) => pane.right?.history || []),
    ],
    x,
    split,
    setSplit,
    active,
    setActive,
    paneWidth,
    panePaths,
    paneActives,
    setPaneWidth,
    switchWorkspaceTab,
    newWorkspaceTab,
    closeWorkspaceTab,
    moveWorkspaceTab,
    openPane,
    closePane,
  }
}
