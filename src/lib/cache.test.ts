import { describe, expect, it, vi } from 'vitest'
import { MemoryCache } from './cache'

describe('MemoryCache', () => {
  it('shares pending work, expires entries, and retries failures', async () => {
    vi.useFakeTimers()
    try {
      const cache = new MemoryCache<number>(2, 100)
      const loader = vi.fn(async () => 42)
      const first = cache.get('a', loader)
      const second = cache.get('a', loader)
      expect(first).toBe(second)
      expect(await first).toBe(42)
      expect(loader).toHaveBeenCalledTimes(1)
      vi.advanceTimersByTime(101)
      expect(await cache.get('a', loader)).toBe(42)
      expect(loader).toHaveBeenCalledTimes(2)

      await expect(
        cache.get('failure', async () => {
          throw Error('temporary')
        }),
      ).rejects.toThrow('temporary')
      expect(await cache.get('failure', async () => 7)).toBe(7)
    } finally {
      vi.useRealTimers()
    }
  })
})
