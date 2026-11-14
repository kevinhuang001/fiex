export interface RetainedViews {
  paths: string[]
  durable: string[]
}

export interface RetainedViewServices {
  native: boolean
  retain(payload: RetainedViews): Promise<unknown>
  schedule(callback: () => void, delay: number): number
  cancel(timer: number): void
}

/** Recent is transient and must not replace the last useful start location. */
export function shouldRememberLastPath(path: string): boolean {
  return path.length > 0 && path !== 'recent://'
}

/** Copy session references into the stable payload sent to the native host. */
export function createRetainedViews(
  paths: readonly string[],
  durable: readonly string[],
): RetainedViews {
  return { paths: [...paths], durable: [...durable] }
}

/** Debounce native retention and return cleanup for the pending request. */
export function scheduleRetainedViews(
  payload: RetainedViews,
  services: RetainedViewServices,
  onError: (message: string) => void,
): () => void {
  if (!services.native) return () => {}
  const timer = services.schedule(() => {
    void services.retain(payload).catch((error) => onError(String(error)))
  }, 150)
  return () => services.cancel(timer)
}
