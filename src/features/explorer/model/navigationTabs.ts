export interface TabHistoryState {
  paths: string[]
  cursor: number
}

export interface NavigationTabState {
  tabs: string[]
  histories: TabHistoryState[]
  active: number
}

export interface NavigationTabDestination extends NavigationTabState {
  path: string
  history: TabHistoryState
}

function copyHistory(history: TabHistoryState): TabHistoryState {
  return { paths: [...history.paths], cursor: history.cursor }
}

function copyHistories(histories: TabHistoryState[], tabs: string[]): TabHistoryState[] {
  return tabs.map((path, index) =>
    histories[index] ? copyHistory(histories[index]!) : { paths: [path], cursor: 0 },
  )
}

function saveCurrentHistory(
  histories: TabHistoryState[],
  active: number,
  currentHistory: TabHistoryState,
): TabHistoryState[] {
  if (active < 0 || active >= histories.length) return histories
  histories[active] = copyHistory(currentHistory)
  return histories
}

/** Adds a tab while saving the active tab's latest back/forward history. */
export function openNavigationTab(input: {
  tabs: string[]
  histories: TabHistoryState[]
  active: number
  currentHistory: TabHistoryState
  path: string
}): NavigationTabDestination {
  const tabs = [...input.tabs, input.path]
  const histories = saveCurrentHistory(
    copyHistories(input.histories, input.tabs),
    input.active,
    input.currentHistory,
  )
  const history = { paths: [input.path], cursor: 0 }
  histories.push(copyHistory(history))
  return { tabs, histories, active: tabs.length - 1, path: input.path, history }
}

/** Switches to a tab and returns its saved path history without mutating input. */
export function selectNavigationTab(input: {
  tabs: string[]
  histories: TabHistoryState[]
  active: number
  currentHistory: TabHistoryState
  index: number
}): NavigationTabDestination | null {
  const path = input.tabs[input.index]
  if (!path || input.index === input.active || input.index < 0) return null

  const histories = saveCurrentHistory(
    copyHistories(input.histories, input.tabs),
    input.active,
    input.currentHistory,
  )
  const history = copyHistory(histories[input.index] ?? { paths: [path], cursor: 0 })
  histories[input.index] = copyHistory(history)
  return {
    tabs: [...input.tabs],
    histories,
    active: input.index,
    path,
    history,
  }
}

/** Closes one tab, selecting its neighbor or restoring the configured start path. */
export function closeNavigationTab(input: {
  tabs: string[]
  histories: TabHistoryState[]
  active: number
  currentHistory: TabHistoryState
  index: number
  startPath: string
}): NavigationTabDestination | null {
  if (input.index < 0 || input.index >= input.tabs.length || !input.tabs[input.index]) return null

  if (input.tabs.length === 1) {
    const history = { paths: [input.startPath], cursor: 0 }
    return {
      tabs: [input.startPath],
      histories: [copyHistory(history)],
      active: 0,
      path: input.startPath,
      history,
    }
  }

  const histories = saveCurrentHistory(
    copyHistories(input.histories, input.tabs),
    input.active,
    input.currentHistory,
  )
  const tabs = [...input.tabs]
  tabs.splice(input.index, 1)
  histories.splice(input.index, 1)
  const active =
    input.index < input.active ? input.active - 1 : Math.min(input.active, tabs.length - 1)
  const path = tabs[active]!
  const history = copyHistory(histories[active] ?? { paths: [path], cursor: 0 })
  histories[active] = copyHistory(history)
  return { tabs, histories, active, path, history }
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
