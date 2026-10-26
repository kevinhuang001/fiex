import { describe, expect, it, vi } from 'vitest'
import { terminalEvents } from './sessionEvents'

describe('terminal startup and asynchronous failures', () => {
  it('keeps an authentication error visible through exit and late readiness', () => {
    const status = vi.fn(),
      error = vi.fn()
    const events = terminalEvents(vi.fn(), status, error)
    events.receive({ type: 'error', message: 'Authentication failed' })
    events.receive({ type: 'exit' })
    events.receive({ type: 'ready' })
    expect(error).toHaveBeenCalledWith('Authentication failed')
    expect(status.mock.calls).toEqual([['error']])
    expect(events.ended()).toBe(true)
  })

  it('waits for readiness or server bytes, preserving banner bytes exactly', () => {
    const write = vi.fn(),
      status = vi.fn()
    const events = terminalEvents(write, status, vi.fn())
    expect(status).not.toHaveBeenCalled()
    const bytes = new TextEncoder().encode('Welcome\r\n')
    events.receive({ type: 'data', data: Array.from(bytes) })
    expect(write).toHaveBeenCalledWith(bytes)
    expect(status).toHaveBeenLastCalledWith('open')
    events.receive({ type: 'exit' })
    expect(status).toHaveBeenLastCalledWith('closed')
  })
})
