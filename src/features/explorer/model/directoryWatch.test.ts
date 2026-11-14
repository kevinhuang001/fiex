import { afterEach, describe, expect, it, vi } from 'vitest'
import { attachDirectoryWatch } from './directoryWatch'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

afterEach(() => vi.useRealTimers())

describe('attachDirectoryWatch', () => {
  it('debounces changes and stops the listener and watcher on cleanup', async () => {
    let emit: (() => void) | undefined
    const unlisten = vi.fn()
    const unwatch = vi.fn(async () => undefined)
    const onChange = vi.fn()
    const stop = attachDirectoryWatch({
      watch: async () => undefined,
      listen: async (handler) => {
        emit = handler
        return unlisten
      },
      unwatch,
      onChange,
      onError: vi.fn(),
      debounceMs: 250,
    })

    await vi.waitFor(() => expect(emit).toBeTypeOf('function'))
    vi.useFakeTimers()
    emit?.()
    vi.advanceTimersByTime(200)
    emit?.()
    vi.advanceTimersByTime(249)
    expect(onChange).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(onChange).toHaveBeenCalledOnce()

    stop()
    stop()
    expect(unlisten).toHaveBeenCalledOnce()
    await vi.waitFor(() => expect(unwatch).toHaveBeenCalledOnce())
    emit?.()
    vi.advanceTimersByTime(250)
    expect(onChange).toHaveBeenCalledOnce()
  })

  it('releases late listener and watcher registrations after cleanup', async () => {
    const watchResult = deferred<void>()
    const listenerResult = deferred<() => void>()
    const unlisten = vi.fn()
    const unwatch = vi.fn(async () => undefined)
    const watch = vi.fn(() => watchResult.promise)
    const listen = vi.fn(() => listenerResult.promise)
    const stop = attachDirectoryWatch({
      watch,
      listen,
      unwatch,
      onChange: vi.fn(),
      onError: vi.fn(),
    })

    await vi.waitFor(() => {
      expect(watch).toHaveBeenCalledOnce()
      expect(listen).toHaveBeenCalledOnce()
    })
    stop()
    expect(unwatch).not.toHaveBeenCalled()

    listenerResult.resolve(unlisten)
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledOnce())
    expect(unwatch).not.toHaveBeenCalled()

    watchResult.resolve()
    await vi.waitFor(() => expect(unwatch).toHaveBeenCalledOnce())
  })
})
