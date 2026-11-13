import type { JsonValue } from '../../../sdk/ui/src/index'
import { normalizeListColumns, type ListColumns } from '../listColumns'
import type { PickerSortState, StateGetArgs } from '../tauriCommandMap'
import type { Location } from '../types'
import { isJsonObject, isUnsignedInteger, onlyKeys, parseJsonValue } from './json'

const LOCATION_KEYS = [
  'name',
  'path',
  'kind',
  'available',
  'total',
  'removable',
  'filesystem',
  'device',
  'mounted',
  'usbfs',
]

function isPickerSortKey(value: unknown): value is PickerSortState['key'] {
  return value === 'name' || value === 'modified' || value === 'size' || value === 'kind'
}

function isPickerSortDirection(value: unknown): value is PickerSortState['direction'] {
  return value === 'asc' || value === 'desc'
}

export function parseLocations(value: JsonValue): Location[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid saved locations')
  const locations: Location[] = []
  for (const item of value) {
    if (
      !isJsonObject(item) ||
      !onlyKeys(item, LOCATION_KEYS) ||
      typeof item.name !== 'string' ||
      typeof item.path !== 'string' ||
      typeof item.kind !== 'string' ||
      !isUnsignedInteger(item.available) ||
      !isUnsignedInteger(item.total) ||
      typeof item.removable !== 'boolean' ||
      typeof item.filesystem !== 'string' ||
      typeof item.device !== 'string' ||
      typeof item.mounted !== 'boolean' ||
      typeof item.usbfs !== 'boolean'
    )
      throw new TypeError('Invalid saved location')
    locations.push({
      name: item.name,
      path: item.path,
      kind: item.kind,
      available: item.available,
      total: item.total,
      removable: item.removable,
      filesystem: item.filesystem,
      device: item.device,
      mounted: item.mounted,
      usbfs: item.usbfs,
    })
  }
  return locations
}

type StateGetValue = ListColumns | Location[] | PickerSortState | JsonValue | null

export function parseStateGetResult(
  args: { namespace: 'layout'; key: string },
  value: unknown,
): ListColumns | null
export function parseStateGetResult(
  args: { namespace: 'explorer'; key: 'locations' },
  value: unknown,
): Location[] | null
export function parseStateGetResult(
  args: { namespace: 'explorer'; key: 'lastPath' },
  value: unknown,
): string | null
export function parseStateGetResult(
  args: { namespace: 'explorer'; key: 'preferences' },
  value: unknown,
): JsonValue | null
export function parseStateGetResult(
  args: { namespace: 'picker'; key: 'sort' },
  value: unknown,
): PickerSortState | null
export function parseStateGetResult(
  args: { namespace: 'picker'; key: 'group' },
  value: unknown,
): 'none' | 'kind' | 'modified' | 'size' | null
export function parseStateGetResult(
  args: { namespace: 'picker'; key: 'lastOpenDirectory' | 'lastSaveDirectory' },
  value: unknown,
): string | null
export function parseStateGetResult(args: StateGetArgs, value: unknown): StateGetValue
export function parseStateGetResult(args: StateGetArgs, value: unknown): StateGetValue {
  if (value === null) return null
  const json = parseJsonValue(value)
  if (args.namespace === 'layout') return normalizeListColumns(json)
  if (args.namespace === 'explorer') {
    if (args.key === 'locations') return parseLocations(json)
    if (args.key === 'lastPath') {
      if (typeof json !== 'string') throw new TypeError('Invalid saved start path')
      return json
    }
    if (args.key === 'preferences') {
      if (!isJsonObject(json)) throw new TypeError('Invalid saved preferences')
      return json
    }
  }
  if (args.namespace === 'picker') {
    if (args.key === 'sort') {
      if (
        !isJsonObject(json) ||
        !onlyKeys(json, ['key', 'direction']) ||
        !isPickerSortKey(json.key) ||
        !isPickerSortDirection(json.direction)
      )
        throw new TypeError('Invalid saved picker sort')
      return { key: json.key, direction: json.direction }
    }
    if (args.key === 'group') {
      if (json !== 'none' && json !== 'kind' && json !== 'modified' && json !== 'size')
        throw new TypeError('Invalid saved picker group')
      return json
    }
    if (typeof json !== 'string') throw new TypeError('Invalid saved picker directory')
    return json
  }
  throw new TypeError('Invalid saved state key')
}
