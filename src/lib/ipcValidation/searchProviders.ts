import type { SearchProviderEngine } from '../types'
import { isJsonObject, onlyKeys } from './json'

function parseSearchProvider(value: unknown): SearchProviderEngine {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['id', 'provider', 'priority', 'builtin', 'modes']) ||
    typeof value.id !== 'string' ||
    !value.id ||
    typeof value.provider !== 'string' ||
    !value.provider ||
    typeof value.priority !== 'number' ||
    !Number.isSafeInteger(value.priority) ||
    typeof value.builtin !== 'boolean' ||
    !Array.isArray(value.modes) ||
    !value.modes.length ||
    !value.modes.every((mode): mode is 'name' | 'content' => mode === 'name' || mode === 'content')
  )
    throw new TypeError('Invalid search_providers IPC result')

  return {
    id: value.id,
    provider: value.provider,
    priority: value.priority,
    builtin: value.builtin,
    modes: [...value.modes],
  }
}

export function parseSearchProviders(value: unknown): SearchProviderEngine[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid search_providers IPC result')
  return value.map(parseSearchProvider)
}
