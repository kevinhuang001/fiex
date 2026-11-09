import type { JsonValue } from '../../../sdk/ui/src/index'
import { normalizeListColumns } from '../listColumns'
import type { StateGetArgs, StateGetResult, ConnectionOpenResult } from '../tauriCommandMap'
import type { Batch, Entry, Listing, Location, OperationResult } from '../types'
import { objectResult, parseJsonValue } from './json'

export function parseEntry(value: JsonValue): Entry {
  if (
    value === null ||
    Array.isArray(value) ||
    typeof value !== 'object' ||
    typeof value.path !== 'string' ||
    typeof value.name !== 'string' ||
    typeof value.isDir !== 'boolean' ||
    typeof value.isSymlink !== 'boolean' ||
    typeof value.size !== 'number' ||
    typeof value.modified !== 'number' ||
    typeof value.created !== 'number' ||
    typeof value.extension !== 'string' ||
    typeof value.mime !== 'string' ||
    !(value.linkTarget === null || typeof value.linkTarget === 'string') ||
    !Array.isArray(value.tags) ||
    !(
      value.sync === null ||
      ['synced', 'partial', 'cloud', 'pinned', 'pending', 'conflict'].includes(String(value.sync))
    )
  )
    throw new TypeError('Invalid file entry from IPC')
  for (const tag of value.tags) {
    if (
      tag === null ||
      Array.isArray(tag) ||
      typeof tag !== 'object' ||
      typeof tag.name !== 'string' ||
      typeof tag.color !== 'number'
    )
      throw new TypeError('Invalid file tag from IPC')
  }
  if (
    value.allowedOps !== undefined &&
    (!Array.isArray(value.allowedOps) || !value.allowedOps.every((op) => typeof op === 'string'))
  )
    throw new TypeError('Invalid file operations from IPC')
  for (const key of ['hasCache', 'pinned', 'uploaded'] as const) {
    if (value[key] !== undefined && typeof value[key] !== 'boolean')
      throw new TypeError(`Invalid ${key} flag from IPC`)
  }
  return value as unknown as Entry
}
export function parseSearchBatch(value: unknown): Batch {
  const batch = objectResult(value)
  if (
    !Array.isArray(batch.entries) ||
    typeof batch.done !== 'boolean' ||
    typeof batch.elapsedMs !== 'number' ||
    typeof batch.truncated !== 'boolean' ||
    typeof batch.skipped !== 'number'
  )
    throw new TypeError('Invalid search batch from IPC')
  const result: Batch = {
    entries: batch.entries.map(parseEntry),
    done: batch.done,
    elapsedMs: batch.elapsedMs,
    truncated: batch.truncated,
    skipped: batch.skipped,
  }
  if (batch.scanned !== undefined && typeof batch.scanned === 'number')
    result.scanned = batch.scanned
  if (batch.incomplete !== undefined && typeof batch.incomplete === 'boolean')
    result.incomplete = batch.incomplete
  if (batch.warning !== undefined && typeof batch.warning === 'string')
    result.warning = batch.warning
  if (batch.engine !== undefined && typeof batch.engine === 'string') result.engine = batch.engine
  if (batch.replace !== undefined && typeof batch.replace === 'boolean')
    result.replace = batch.replace
  return result
}

export function parseListing(value: unknown): Listing {
  const listing = objectResult(value)
  if (
    typeof listing.path !== 'string' ||
    !Array.isArray(listing.entries) ||
    !Array.isArray(listing.allowedOps) ||
    !listing.allowedOps.every((operation) => typeof operation === 'string') ||
    !Array.isArray(listing.warnings) ||
    !listing.warnings.every((warning) => typeof warning === 'string') ||
    (listing.virtualVolume !== undefined && typeof listing.virtualVolume !== 'boolean') ||
    (listing.defaultView !== undefined &&
      !['list', 'gallery', 'columns', 'icons'].includes(String(listing.defaultView)))
  )
    throw new TypeError('Invalid directory listing from IPC')
  return {
    path: listing.path,
    entries: listing.entries.map(parseEntry),
    allowedOps: listing.allowedOps,
    warnings: listing.warnings,
    ...(typeof listing.virtualVolume === 'boolean' ? { virtualVolume: listing.virtualVolume } : {}),
    ...(typeof listing.defaultView === 'string'
      ? { defaultView: listing.defaultView as Listing['defaultView'] }
      : {}),
  }
}

export function parseOperationResult(value: unknown): OperationResult {
  const result = objectResult(value)
  if (
    !Array.isArray(result.paths) ||
    !result.paths.every((path) => typeof path === 'string') ||
    !Array.isArray(result.errors) ||
    !result.errors.every((error) => typeof error === 'string') ||
    typeof result.skipped !== 'number'
  )
    throw new TypeError('Invalid file operation result from IPC')
  return result as unknown as OperationResult
}
export function parseConnectionOpenResult(value: unknown): ConnectionOpenResult {
  const result = objectResult(value)
  if (result.kind === 'vfs' && typeof result.path === 'string')
    return { kind: 'vfs', path: result.path }
  if (
    result.kind === 'pluginView' &&
    typeof result.pluginId === 'string' &&
    typeof result.connectionId === 'string' &&
    typeof result.assetPath === 'string' &&
    'state' in result &&
    (result.tabTitle === undefined || typeof result.tabTitle === 'string')
  ) {
    return {
      kind: 'pluginView',
      pluginId: result.pluginId,
      connectionId: result.connectionId,
      assetPath: result.assetPath,
      state: result.state,
      ...(typeof result.tabTitle === 'string' ? { tabTitle: result.tabTitle } : {}),
    }
  }
  throw new TypeError('Invalid connection navigation response')
}
export function parseLocations(value: JsonValue): Location[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid saved locations')
  for (const item of value) {
    if (
      item === null ||
      Array.isArray(item) ||
      typeof item !== 'object' ||
      typeof item.name !== 'string' ||
      typeof item.path !== 'string' ||
      typeof item.kind !== 'string' ||
      typeof item.available !== 'number' ||
      typeof item.total !== 'number' ||
      typeof item.removable !== 'boolean' ||
      typeof item.filesystem !== 'string' ||
      typeof item.device !== 'string' ||
      typeof item.mounted !== 'boolean' ||
      typeof item.usbfs !== 'boolean'
    )
      throw new TypeError('Invalid saved location')
  }
  return value as unknown as Location[]
}
export function parseStateGetResult<A extends StateGetArgs>(
  args: A,
  value: unknown,
): StateGetResult<A> {
  if (value === null) return null as StateGetResult<A>
  const json = parseJsonValue(value)
  if (args.namespace === 'layout') {
    return normalizeListColumns(json) as StateGetResult<A>
  }
  if (args.namespace === 'explorer') {
    if (args.key === 'locations') return parseLocations(json) as StateGetResult<A>
    if (args.key === 'lastPath') {
      if (typeof json !== 'string') throw new TypeError('Invalid saved start path')
      return json as unknown as StateGetResult<A>
    }
    if (args.key === 'preferences') {
      if (json === null || Array.isArray(json) || typeof json !== 'object')
        throw new TypeError('Invalid saved preferences')
      return json as StateGetResult<A>
    }
  }
  if (args.namespace === 'picker') {
    if (args.key === 'sort') {
      if (
        json === null ||
        Array.isArray(json) ||
        typeof json !== 'object' ||
        !['name', 'modified', 'size', 'kind'].includes(String(json.key)) ||
        !['asc', 'desc'].includes(String(json.direction))
      )
        throw new TypeError('Invalid saved picker sort')
      return { key: json.key, direction: json.direction } as StateGetResult<A>
    }
    if (args.key === 'group') {
      if (!['none', 'kind', 'modified', 'size'].includes(String(json)))
        throw new TypeError('Invalid saved picker group')
      return json as StateGetResult<A>
    }
    if (typeof json !== 'string') throw new TypeError('Invalid saved picker directory')
    return json as StateGetResult<A>
  }
  return json as StateGetResult<A>
}
