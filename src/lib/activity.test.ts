import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  ACTIVITY_REVEAL_DELAY_MS,
  activityEta,
  formatRemaining,
  scheduleActivityReveal,
} from './activity'

describe('delayed activity centre reveal', () => {
  afterEach(() => vi.useRealTimers())

  it('waits one second before revealing a still-running task', () => {
    vi.useFakeTimers()
    const reveal = vi.fn()
    const timer = scheduleActivityReveal(() => true, reveal)

    vi.advanceTimersByTime(ACTIVITY_REVEAL_DELAY_MS - 1)
    expect(reveal).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(reveal).toHaveBeenCalledOnce()
    clearTimeout(timer)
  })

  it('does not reveal a task that finished before the delay', () => {
    vi.useFakeTimers()
    let running = true
    const reveal = vi.fn()
    scheduleActivityReveal(() => running, reveal)
    running = false

    vi.advanceTimersByTime(ACTIVITY_REVEAL_DELAY_MS)
    expect(reveal).not.toHaveBeenCalled()
  })
})

describe('activity progress formatting', () => {
  it('formats seconds, minutes and hours', () => {
    expect(formatRemaining(1)).toBe('1 秒')
    expect(formatRemaining(59)).toBe('59 秒')
    expect(formatRemaining(60)).toBe('1 分 0 秒')
    expect(formatRemaining(65)).toBe('1 分 5 秒')
    expect(formatRemaining(3600)).toBe('1 小时 0 分')
    expect(formatRemaining(3665)).toBe('1 小时 1 分')
  })

  it('estimates remaining time from the completed count', () => {
    const startedAt = 1_000_000
    expect(
      activityEta({ status: 'running', total: 10, done: 2, startedAt }, startedAt + 2_000),
    ).toBe('8 秒')
    expect(
      activityEta({ status: 'running', total: 100, done: 50, startedAt }, startedAt + 60_000),
    ).toBe('1 分 0 秒')
  })

  it('does not estimate when there is no usable progress', () => {
    const startedAt = 1_000_000
    expect(activityEta({ status: 'done', total: 10, done: 5, startedAt }, startedAt + 2_000)).toBe(
      null,
    )
    expect(
      activityEta({ status: 'running', total: 10, done: 10, startedAt }, startedAt + 2_000),
    ).toBe(null)
    expect(
      activityEta({ status: 'running', total: 10, done: 0, startedAt }, startedAt + 2_000),
    ).toBe(null)
    expect(activityEta({ status: 'running', total: undefined, done: 5, startedAt })).toBe(null)
    expect(activityEta({ status: 'running', total: 10, done: 1, startedAt }, startedAt + 10)).toBe(
      null,
    )
  })
})
