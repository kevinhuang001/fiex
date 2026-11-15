export function mergePaneSelections(
  split: boolean,
  leftSelection: string[],
  rightSelection: string[],
): string[] {
  return split ? [...new Set([...leftSelection, ...rightSelection])] : leftSelection
}

export function selectedEntriesAcrossPanes<T extends { path: string }>(
  split: boolean,
  leftEntries: T[],
  rightEntries: T[],
  singlePaneEntries: T[],
  selectedPaths: string[],
): T[] {
  if (!split) return singlePaneEntries

  const selected = new Set(selectedPaths)
  const entries = new Map<string, T>()
  for (const entry of [...leftEntries, ...rightEntries]) {
    if (selected.has(entry.path)) entries.set(entry.path, entry)
  }
  return [...entries.values()]
}
