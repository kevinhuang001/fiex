export interface DirectoryWatchOptions {
  watch: () => Promise<unknown>
  listen: (onChange: () => void) => Promise<() => void>
  unwatch: () => Promise<unknown>
  onChange: () => void
  onError: (error: unknown) => void
  debounceMs?: number
}

/** Keeps native watch registration and its event listener within one lifecycle. */
export function attachDirectoryWatch({
  watch,
  listen,
  unwatch,
  onChange,
  onError,
  debounceMs = 250,
}: DirectoryWatchOptions): () => void {
  let active = true
  let disposed = false
  let watchStarted = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let unlisten: (() => void) | undefined

  const handleChange = () => {
    if (!active) return
    if (timer !== undefined) clearTimeout(timer)
    timer = setTimeout(() => {
      timer = undefined
      if (active) onChange()
    }, debounceMs)
  }

  const watchReady = Promise.resolve()
    .then(() => {
      if (!active) return
      watchStarted = true
      return watch()
    })
    .catch((error: unknown) => {
      if (active) onError(error)
    })

  void Promise.resolve()
    .then(() => {
      if (!active) return
      return listen(handleChange)
    })
    .then((stop) => {
      if (!stop) return
      if (active) unlisten = stop
      else stop()
    })
    .catch((error: unknown) => {
      if (active) onError(error)
    })

  return () => {
    if (disposed) return
    disposed = true
    active = false
    if (timer !== undefined) clearTimeout(timer)
    try {
      unlisten?.()
    } catch {
      // Continue releasing the native watch even if listener teardown fails.
    }
    unlisten = undefined
    void watchReady.then(() => (watchStarted ? unwatch() : undefined)).catch(() => {})
  }
}
