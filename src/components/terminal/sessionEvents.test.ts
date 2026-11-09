import { describe, expect, it, vi } from 'vitest'
import { parseTerminalEvent, terminalEvents } from './sessionEvents'

describe('terminal startup and asynchronous failures', () => {
  it('validates terminal wire events before dispatch', () => {
    expect(parseTerminalEvent({ type: 'data', data: [0, 10, 255] })).toEqual({
      type: 'data',
      data: [0, 10, 255],
    })
    expect(() => parseTerminalEvent({ type: 'data', data: [256] })).toThrow(
      'Invalid terminal event',
    )
    expect(() => parseTerminalEvent({ type: 'error', message: 3 })).toThrow(
      'Invalid terminal event',
    )
  })

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
