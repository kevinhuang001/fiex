/** Bounded in-memory cache for presentation data. Pending requests share one IPC call.
 * Failed requests are removed so a transient failure can be retried immediately. */
export class MemoryCache<V> {
  private values = new Map<string, { value: Promise<V>; expires: number; resolved?: V }>()

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
    const item: { value: Promise<V>; expires: number; resolved?: V } = {
      value,
      expires: now + this.ttlMs,
      resolved: hit?.resolved,
    }
    void value.then(
      (resolved) => {
        item.resolved = resolved
      },
      () => {},
    )
    this.values.set(key, item)
    while (this.values.size > this.limit) this.values.delete(this.values.keys().next().value!)
    void value.catch(() => {
      if (this.values.get(key) === item) this.values.delete(key)
    })
    return value
  }

  peek(key: string): V | undefined {
    const item = this.values.get(key)
    return item?.resolved
  }

  clear() {
    this.values.clear()
  }
}
