export interface TabHistoryState {
  paths: string[]
  cursor: number
}

export interface NavigationTabState {
  tabs: string[]
  histories: TabHistoryState[]
  active: number
}

/** Reorders tabs and their navigation histories without mutating pane state. */
export function moveNavigationTab(input: {
  tabs: string[]
  histories: TabHistoryState[]
  active: number
  currentHistory: TabHistoryState
  from: number
  to: number
}): NavigationTabState | null {
  const movedTab = input.tabs[input.from]
  const destination = input.tabs[input.to]
  if (input.from === input.to || !movedTab || !destination) return null

  const tabs = [...input.tabs]
  tabs.splice(input.to, 0, tabs.splice(input.from, 1)[0]!)

  const histories = input.histories.map((history) => ({
    paths: [...history.paths],
    cursor: history.cursor,
  }))
  histories[input.active] = {
    paths: [...input.currentHistory.paths],
    cursor: input.currentHistory.cursor,
  }
  histories.splice(input.to, 0, histories.splice(input.from, 1)[0]!)

  const active =
    input.active === input.from
      ? input.to
      : input.from < input.active && input.to >= input.active
        ? input.active - 1
        : input.from > input.active && input.to <= input.active
          ? input.active + 1
          : input.active

  return { tabs, histories, active }
}
