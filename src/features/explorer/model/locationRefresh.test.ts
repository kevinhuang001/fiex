import { describe, expect, it, vi } from 'vitest'
import type { Location } from '@/lib/types'
import { createLocationRefreshCoordinator } from './locationRefresh'

function location(name: string, path: string): Location {
  return {
    name,
    path,
    kind: 'folder',
    available: 0,
    total: 0,
    removable: false,
    filesystem: '',
    device: '',
    mounted: true,
    usbfs: false,
  }
}

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  const promise = new Promise<T>((finish) => {
    resolve = finish
  })
  return { promise, resolve }
}

describe('Location refresh coordination', () => {
  it('shares an in-flight query and publishes the result once', async () => {
    const response = deferred<Location[]>()
    const load = vi.fn(() => response.promise)
    const onChange = vi.fn()
    const refresh = createLocationRefreshCoordinator(load, onChange)

    const first = refresh.refresh()
    const second = refresh.refresh()
    expect(second).toBe(first)
    expect(load).toHaveBeenCalledTimes(1)

    const next = [location('Documents', '/home/Documents')]
    response.resolve(next)
    await Promise.all([first, second])
    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith(next)
  })

  it('lets a forced refresh supersede an older response', async () => {
    const older = deferred<Location[]>()
    const newer = deferred<Location[]>()
    const load = vi.fn().mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise)
    const onChange = vi.fn()
    const refresh = createLocationRefreshCoordinator(load, onChange)

    const oldRequest = refresh.refresh()
    const newRequest = refresh.refresh(true)
    const latest = [location('Latest', '/latest')]
    newer.resolve(latest)
    await newRequest
    older.resolve([location('Stale', '/stale')])
    await oldRequest

    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith(latest)
  })

  it('does not publish structurally identical location results twice', async () => {
    const next = [location('Documents', '/home/Documents')]
    const onChange = vi.fn()
    const refresh = createLocationRefreshCoordinator(async () => next, onChange)

    await refresh.refresh()
    await refresh.refresh(true)

    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith(next)
  })
})
