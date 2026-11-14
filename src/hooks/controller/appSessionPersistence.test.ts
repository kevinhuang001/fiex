import { describe, expect, it, vi } from 'vitest'
import {
  createRetainedViews,
  scheduleRetainedViews,
  shouldRememberLastPath,
  type RetainedViewServices,
} from './appSessionPersistence'

describe('app session persistence', () => {
  it('does not replace the remembered path with an empty or Recent location', () => {
    expect(shouldRememberLastPath('')).toBe(false)
    expect(shouldRememberLastPath('recent://')).toBe(false)
    expect(shouldRememberLastPath('/home/kevin/Documents')).toBe(true)
    expect(shouldRememberLastPath('applications://user')).toBe(true)
  })

  it('copies session and durable references without reordering them', () => {
    const paths = ['file:///one', 'file:///two']
    const durable = ['file:///favorite', 'file:///workspace']
    const payload = createRetainedViews(paths, durable)

    expect(payload).toEqual({ paths, durable })
    expect(payload.paths).not.toBe(paths)
    expect(payload.durable).not.toBe(durable)
  })

  it('skips native retention in the browser', () => {
    const services: RetainedViewServices = {
      native: false,
      retain: vi.fn(),
      schedule: vi.fn(),
      cancel: vi.fn(),
    }

    scheduleRetainedViews({ paths: [], durable: [] }, services, vi.fn())()

    expect(services.schedule).not.toHaveBeenCalled()
    expect(services.retain).not.toHaveBeenCalled()
  })

  it('schedules the latest payload once and cancels it on cleanup', async () => {
    let run: () => void = () => {}
    const cancel = vi.fn()
    const onError = vi.fn()
    const services: RetainedViewServices = {
      native: true,
      retain: vi.fn().mockRejectedValue(new Error('native failed')),
      schedule: (callback, delay) => {
        expect(delay).toBe(150)
        run = callback
        return 19
      },
      cancel,
    }
    const payload = { paths: ['file:///current'], durable: ['file:///favorite'] }

    const cleanup = scheduleRetainedViews(payload, services, onError)
    run()
    await Promise.resolve()
    cleanup()

    expect(services.retain).toHaveBeenCalledWith(payload)
    expect(onError).toHaveBeenCalledWith('Error: native failed')
    expect(cancel).toHaveBeenCalledWith(19)
  })
})
