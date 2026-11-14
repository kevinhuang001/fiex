export interface NavigationHistory {
  history: string[]
  cursor: number
}

export interface NavigationHistoryTarget {
  path: string
  cursor: number
}

/** Adds a path after the current history entry and drops any forward branch. */
export function appendNavigationPath(
  history: string[],
  cursor: number,
  next: string,
): NavigationHistory {
  const paths = [...history.slice(0, cursor + 1), next]
  return { history: paths, cursor: paths.length - 1 }
}

/** Resolves a one-step back or forward action without changing its history. */
export function moveNavigationCursor(
  history: string[],
  cursor: number,
  direction: -1 | 1,
): NavigationHistoryTarget | null {
  if (cursor < 0 || cursor >= history.length) return null
  const nextCursor = cursor + direction
  const path = history[nextCursor]
  return path === undefined ? null : { path, cursor: nextCursor }
}

export function previousNavigationPath(history: string[], cursor: number): string | null {
  return moveNavigationCursor(history, cursor, -1)?.path ?? null
}
