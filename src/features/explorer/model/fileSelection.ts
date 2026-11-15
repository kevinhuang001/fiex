export interface VisiblePathSelectionInput {
  visiblePaths: readonly string[]
  selectedPaths: readonly string[]
  clickedPath: string
  anchorPath: string | null
  extendRange: boolean
  additive: boolean
}

export interface VisiblePathSelection {
  selectedPaths: string[]
  anchorPath: string
}

export function selectVisiblePath(input: VisiblePathSelectionInput): VisiblePathSelection {
  const { visiblePaths, selectedPaths, clickedPath, anchorPath } = input
  if (input.extendRange) {
    const anchorIndex = anchorPath === null ? -1 : visiblePaths.indexOf(anchorPath)
    const clickedIndex = visiblePaths.indexOf(clickedPath)
    if (anchorPath === null || anchorIndex < 0 || clickedIndex < 0)
      return { selectedPaths: [clickedPath], anchorPath: clickedPath }
    return {
      selectedPaths: visiblePaths.slice(
        Math.min(anchorIndex, clickedIndex),
        Math.max(anchorIndex, clickedIndex) + 1,
      ),
      anchorPath,
    }
  }

  if (input.additive) {
    const next = selectedPaths.includes(clickedPath)
      ? selectedPaths.filter((path) => path !== clickedPath)
      : [...selectedPaths, clickedPath]
    return { selectedPaths: next, anchorPath: clickedPath }
  }

  return { selectedPaths: [clickedPath], anchorPath: clickedPath }
}

export function selectAllVisiblePaths(visiblePaths: readonly string[]): string[] {
  return [...visiblePaths]
}

export interface KeyboardPathSelectionInput {
  visiblePaths: readonly string[]
  selectedPaths: readonly string[]
  key: string
  extendSelection: boolean
  disabled: boolean
}

export interface KeyboardPathSelection {
  focusedPath: string
  selectedPaths: string[]
}

export function moveVisiblePathSelection(
  input: KeyboardPathSelectionInput,
): KeyboardPathSelection | null {
  if (input.disabled || input.visiblePaths.length === 0) return null
  if (
    input.key !== 'ArrowDown' &&
    input.key !== 'ArrowUp' &&
    input.key !== 'ArrowLeft' &&
    input.key !== 'ArrowRight' &&
    input.key !== 'Home' &&
    input.key !== 'End'
  )
    return null

  const currentIndex = input.visiblePaths.indexOf(input.selectedPaths.at(-1) ?? '')
  const nextIndex =
    input.key === 'Home'
      ? 0
      : input.key === 'End'
        ? input.visiblePaths.length - 1
        : currentIndex < 0
          ? 0
          : Math.max(
              0,
              Math.min(
                input.visiblePaths.length - 1,
                currentIndex + (input.key === 'ArrowUp' || input.key === 'ArrowLeft' ? -1 : 1),
              ),
            )
  const focusedPath = input.visiblePaths[nextIndex]
  if (focusedPath === undefined) return null
  return {
    focusedPath,
    selectedPaths: input.extendSelection
      ? [...new Set([...input.selectedPaths, focusedPath])]
      : [focusedPath],
  }
}
