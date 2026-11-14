import { describe, expect, it } from 'vitest'
import { createSearchLifecycle } from './searchLifecycle'

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: Error) => void = () => {}
  const promise = new Promise<T>((finish, fail) => {
    resolve = finish
    reject = fail
  })
  return { promise, resolve, reject }
}

describe('Search lifecycle', () => {
  it('invalidates previous result generations on a new search or cleanup', () => {
    const lifecycle = createSearchLifecycle()
    const previous = lifecycle.begin()
    expect(lifecycle.isCurrent(previous)).toBe(true)

    const current = lifecycle.begin()
    expect(lifecycle.isCurrent(previous)).toBe(false)
    expect(lifecycle.isCurrent(current)).toBe(true)

    lifecycle.invalidate()
    expect(lifecycle.isCurrent(current)).toBe(false)
  })

  it('serializes cancellation requests and recovers after one rejects', async () => {
    const lifecycle = createSearchLifecycle()
    const blocked = deferred<void>()
    const calls: string[] = []

    const first = lifecycle.cancel(() => {
      calls.push('first')
      return blocked.promise
    })
    const second = lifecycle.cancel(async () => {
      calls.push('second')
    })
    await Promise.resolve()
    expect(calls).toEqual(['first'])

    blocked.reject(new Error('cancel failed'))
    await Promise.all([first, second])
    expect(calls).toEqual(['first', 'second'])
  })
})
