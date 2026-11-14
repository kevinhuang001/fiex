import { describe, expect, it } from 'vitest'
import { createNavigationRequestLifecycle } from './navigationRequest'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((done, fail) => {
    resolve = done
    reject = fail
  })
  return { promise, resolve, reject }
}

describe('NavigationRequestLifecycle', () => {
  it('ignores a parent lookup after a new location is committed', async () => {
    const lifecycle = createNavigationRequestLifecycle()
    const request = deferred<string>()
    const result = lifecycle.run(() => request.promise)

    lifecycle.invalidate()
    request.resolve('/old-parent')

    await expect(result).resolves.toEqual({ status: 'stale' })
  })

  it('keeps only the newest parent request result', async () => {
    const lifecycle = createNavigationRequestLifecycle()
    const older = deferred<string>()
    const newer = deferred<string>()
    const olderResult = lifecycle.run(() => older.promise)
    const newerResult = lifecycle.run(() => newer.promise)

    newer.resolve('/current-parent')
    await expect(newerResult).resolves.toEqual({ status: 'resolved', value: '/current-parent' })

    older.resolve('/stale-parent')
    await expect(olderResult).resolves.toEqual({ status: 'stale' })
  })

  it('does not surface an obsolete parent lookup error', async () => {
    const lifecycle = createNavigationRequestLifecycle()
    const request = deferred<string>()
    const result = lifecycle.run(() => request.promise)

    lifecycle.invalidate()
    request.reject(new Error('old location disappeared'))

    await expect(result).resolves.toEqual({ status: 'stale' })
  })
})
