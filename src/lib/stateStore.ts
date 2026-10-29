import { call, native } from './api'

/** The Core JSON store is authoritative. The browser mirror is only a
 * synchronous first-paint snapshot; callers never address localStorage. */
const prefix = 'fiex:state:'
function readJson<T>(raw: string | null): T | null {
  try { return raw === null ? null : JSON.parse(raw) as T } catch { return null }
}
export function stateSnapshot<T>(key: string, legacyKey?: string): T | null {
  const cached = readJson<T>(localStorage.getItem(prefix + key))
  if (cached !== null) return cached
  if (!legacyKey) return null
  const legacy = localStorage.getItem(legacyKey)
  if (legacy === null) return null
  if (legacyKey === 'fiex:preferences' || legacyKey === 'fiex.picker.sort')
    return readJson<T>(legacy)
  return legacy as T
}
export async function loadState<T>(key: string, legacyKey?: string): Promise<T | null> {
  const snapshot = stateSnapshot<T>(key, legacyKey)
  if (!native) return snapshot
  const saved = await call<T | null>('state_get', { key })
  const value = saved ?? snapshot
  if (value !== null) {
    localStorage.setItem(prefix + key, JSON.stringify(value))
    if (saved === null) await call('state_set', { key, value })
    if (legacyKey) localStorage.removeItem(legacyKey)
  }
  return value
}
export function saveState<T>(key: string, value: T): void {
  // Immediate per-window mirror for pre-hydration startup; no second policy.
  localStorage.setItem(prefix + key, JSON.stringify(value))
  if (native) {
    void call('state_set', { key, value }).catch((error) => {
      window.dispatchEvent(new CustomEvent('fiex:state-error', { detail: String(error) }))
    })
  }
}
