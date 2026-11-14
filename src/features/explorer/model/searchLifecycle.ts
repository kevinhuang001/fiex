export interface SearchLifecycle {
  begin(): number
  invalidate(): void
  isCurrent(generation: number): boolean
  cancel(action: () => Promise<unknown>): Promise<void>
}

/** Tracks stale-result generations and serializes cancellations for one pane. */
export function createSearchLifecycle(): SearchLifecycle {
  let generation = 0
  let cancellations: Promise<void> = Promise.resolve()

  return {
    begin() {
      generation += 1
      return generation
    },
    invalidate() {
      generation += 1
    },
    isCurrent(candidate) {
      return candidate === generation
    },
    cancel(action) {
      cancellations = cancellations
        .then(async () => {
          await action()
        })
        .catch(() => {})
      return cancellations
    },
  }
}
