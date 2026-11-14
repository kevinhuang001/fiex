export type NavigationRequestResult<T> =
  { status: 'resolved'; value: T } | { status: 'failed'; error: unknown } | { status: 'stale' }

export interface NavigationRequestLifecycle {
  invalidate(): void
  run<T>(request: () => Promise<T>): Promise<NavigationRequestResult<T>>
}

/** Rejects asynchronous path results once a newer request or navigation wins. */
export function createNavigationRequestLifecycle(): NavigationRequestLifecycle {
  let generation = 0

  return {
    invalidate() {
      generation += 1
    },
    async run<T>(request: () => Promise<T>): Promise<NavigationRequestResult<T>> {
      const current = ++generation
      try {
        const value = await request()
        return current === generation ? { status: 'resolved', value } : { status: 'stale' }
      } catch (error) {
        return current === generation ? { status: 'failed', error } : { status: 'stale' }
      }
    },
  }
}
