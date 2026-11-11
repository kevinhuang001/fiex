import { describe, expect, it, vi } from 'vitest'
import { PluginTaskRefreshTracker } from './pluginTaskRefresh'

describe('PluginTaskRefreshTracker', () => {
  it('refreshes the invoking directory after successful completion', () => {
    const refresh = vi.fn()
    const tracker = new PluginTaskRefreshTracker(refresh)

    tracker.register('task-1', '/documents')
    tracker.settle('task-1', 'running')
    expect(refresh).not.toHaveBeenCalled()
    tracker.settle('task-1', 'completed')

    expect(refresh).toHaveBeenCalledWith(['/documents'])
  })

  it.each(['failed', 'cancelled'])('does not refresh after %s tasks', (status) => {
    const refresh = vi.fn()
    const tracker = new PluginTaskRefreshTracker(refresh)

    tracker.register('task-1', '/documents')
    tracker.settle('task-1', status)

    expect(refresh).not.toHaveBeenCalled()
  })

  it('handles a fast task that finishes before execute returns its id', () => {
    const refresh = vi.fn()
    const tracker = new PluginTaskRefreshTracker(refresh)

    tracker.settle('task-1', 'completed')
    tracker.register('task-1', '/documents')

    expect(refresh).toHaveBeenCalledWith(['/documents'])
  })

  it('expires unmatched completion events', () => {
    const refresh = vi.fn()
    let now = 0
    const tracker = new PluginTaskRefreshTracker(refresh, () => now)

    tracker.settle('task-1', 'completed')
    now = 60_001
    tracker.register('task-1', '/documents')

    expect(refresh).not.toHaveBeenCalled()
  })
})
