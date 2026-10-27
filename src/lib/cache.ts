/** Bounded in-memory cache for presentation data. Pending requests share one IPC call.
 * Failed requests are removed so a transient failure can be retried immediately. */
export class MemoryCache<V> {
  private values = new Map<string, { value: Promise<V>; expires: number }>()

  constructor(
    private readonly limit: number,
    private readonly ttlMs: number,
  ) {}

  get(key: string, load: () => Promise<V>): Promise<V> {
    const now = Date.now()
    const hit = this.values.get(key)
    if (hit && hit.expires > now) {
      this.values.delete(key)
      this.values.set(key, hit)
      return hit.value
    }
    this.values.delete(key)
    const value = Promise.resolve().then(load)
    const item = { value, expires: now + this.ttlMs }
    this.values.set(key, item)
    while (this.values.size > this.limit) this.values.delete(this.values.keys().next().value!)
    void value.catch(() => {
      if (this.values.get(key) === item) this.values.delete(key)
    })
    return value
  }

  clear() {
    this.values.clear()
  }
}
