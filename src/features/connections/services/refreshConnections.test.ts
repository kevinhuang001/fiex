import type { RemoteConnection } from '@/lib/types'
import { describe, expect, it, vi } from 'vitest'
import { refreshConnections } from './refreshConnections'

describe('refreshConnections', () => {
  it('stores a copy of the returned connection list and marks it loaded', async () => {
    const connections = [{ id: 'ssh-one' }] as RemoteConnection[]
    const setConnections = vi.fn()
    const setConnectionsLoaded = vi.fn()

    await refreshConnections(async () => connections, setConnections, setConnectionsLoaded)

    expect(setConnections).toHaveBeenCalledWith(connections)
    expect(setConnections.mock.calls[0]?.[0]).not.toBe(connections)
    expect(setConnectionsLoaded).toHaveBeenCalledWith(true)
  })

  it('clears the list when the response is not an array', async () => {
    const setConnections = vi.fn()
    const setConnectionsLoaded = vi.fn()

    await refreshConnections(async () => ({ items: [] }), setConnections, setConnectionsLoaded)

    expect(setConnections).toHaveBeenCalledWith([])
    expect(setConnectionsLoaded).toHaveBeenCalledWith(true)
  })

  it('clears the list and marks loading complete after a request failure', async () => {
    const setConnections = vi.fn()
    const setConnectionsLoaded = vi.fn()

    await refreshConnections(
      async () => {
        throw new Error('offline')
      },
      setConnections,
      setConnectionsLoaded,
    )

    expect(setConnections).toHaveBeenCalledWith([])
    expect(setConnectionsLoaded).toHaveBeenCalledWith(true)
  })
})
