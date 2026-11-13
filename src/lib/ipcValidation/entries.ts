import type { JsonValue } from '../../../sdk/ui/src/index'
import { normalizeListColumns } from '../listColumns'
import type { StateGetArgs, StateGetResult, ConnectionOpenResult } from '../tauriCommandMap'
import type { Batch, Entry, Listing, Location, OperationResult } from '../types'
import { isJsonObject, objectResult, parseJsonValue } from './json'

const ENTRY_KEYS = [
  'path',
  'name',
  'isDir',
  'isSymlink',
  'size',
  'modified',
  'created',
  'extension',
  'mime',
  'linkTarget',
  'allowedOps',
  'tags',
  'hasCache',
  'pinned',
  'uploaded',
  'sync',
]

function onlyKeys(value: Record<string, unknown>, allowed: string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function unsignedInteger(value: unknown): value is number {
  return (
    typeof value === 'number' && Number.isFinite(value) && Number.isInteger(value) && value >= 0
  )
}

function isEntrySync(value: unknown): value is Entry['sync'] {
  return (
    value === null ||
    (typeof value === 'string' &&
      ['synced', 'partial', 'cloud', 'pinned', 'pending', 'conflict'].includes(value))
  )
}

function parseEntryTag(value: unknown): Entry['tags'][number] {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['name', 'color']) ||
    typeof value.name !== 'string' ||
    !unsignedInteger(value.color) ||
    value.color > 7
  )
    throw new TypeError('Invalid file tag from IPC')
  return { name: value.name, color: value.color }
}

export function parseEntry(value: JsonValue): Entry {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ENTRY_KEYS) ||
    typeof value.path !== 'string' ||
    typeof value.name !== 'string' ||
    typeof value.isDir !== 'boolean' ||
    typeof value.isSymlink !== 'boolean' ||
    !unsignedInteger(value.size) ||
    !unsignedInteger(value.modified) ||
    !unsignedInteger(value.created) ||
    typeof value.extension !== 'string' ||
    typeof value.mime !== 'string' ||
    !(value.linkTarget === null || typeof value.linkTarget === 'string') ||
    !Array.isArray(value.tags) ||
    !Array.isArray(value.allowedOps) ||
    !value.allowedOps.every((operation) => typeof operation === 'string') ||
    typeof value.hasCache !== 'boolean' ||
    typeof value.pinned !== 'boolean' ||
    typeof value.uploaded !== 'boolean' ||
    !isEntrySync(value.sync)
  )
    throw new TypeError('Invalid file entry from IPC')
  return {
    path: value.path,
    name: value.name,
    isDir: value.isDir,
    isSymlink: value.isSymlink,
    size: value.size,
    modified: value.modified,
    created: value.created,
    extension: value.extension,
    mime: value.mime,
    linkTarget: value.linkTarget,
    allowedOps: value.allowedOps,
    tags: value.tags.map(parseEntryTag),
    hasCache: value.hasCache,
    pinned: value.pinned,
    uploaded: value.uploaded,
    sync: value.sync,
  }
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
