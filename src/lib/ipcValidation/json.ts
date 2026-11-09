import type { JsonValue } from '../../../sdk/ui/src/index'

type JsonObject = { [key: string]: JsonValue }

export function isJsonObject(value: unknown): value is Record<string, JsonValue> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

export function parseJsonValue(value: unknown, seen = new Set<object>()): JsonValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value !== 'object' || seen.has(value)) throw new TypeError('Invalid JSON IPC value')
  seen.add(value)
  try {
    if (Array.isArray(value)) return value.map((item) => parseJsonValue(item, seen))
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null)
      throw new TypeError('Invalid JSON IPC object')
    const result: JsonObject = Object.create(null)
    for (const [key, entry] of Object.entries(value)) result[key] = parseJsonValue(entry, seen)
    return result
  } finally {
    seen.delete(value)
  }
}

export function objectResult(value: unknown): Record<string, JsonValue> {
  const json = parseJsonValue(value)
  if (json === null || Array.isArray(json) || typeof json !== 'object')
    throw new TypeError('Expected an object from IPC')
  return json
}
