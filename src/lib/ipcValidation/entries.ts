import type { JsonValue } from '../../../sdk/ui/src/index'
import { normalizeListColumns } from '../listColumns'
import type { StateGetArgs, StateGetResult, ConnectionOpenResult } from '../tauriCommandMap'
import type { Batch, Entry, Listing, Location } from '../types'
import { isJsonObject, isUnsignedInteger, objectResult, onlyKeys, parseJsonValue } from './json'

export { parseOperationResult } from './operations'

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
const LISTING_KEYS = ['allowedOps', 'virtualVolume', 'path', 'entries', 'warnings', 'defaultView']
const SEARCH_BATCH_KEYS = [
  'entries',
  'done',
  'elapsedMs',
  'truncated',
  'skipped',
  'scanned',
  'incomplete',
  'warning',
  'engine',
  'replace',
]

function isEntrySync(value: unknown): value is Entry['sync'] {
  return (
    value === null ||
    (typeof value === 'string' &&
      ['synced', 'partial', 'cloud', 'pinned', 'pending', 'conflict'].includes(value))
  )
}

function isListingView(value: unknown): value is NonNullable<Listing['defaultView']> {
  return value === 'list' || value === 'gallery' || value === 'columns' || value === 'icons'
}

function parseEntryTag(value: unknown): Entry['tags'][number] {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['name', 'color']) ||
    typeof value.name !== 'string' ||
    !isUnsignedInteger(value.color) ||
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
    !isUnsignedInteger(value.size) ||
    !isUnsignedInteger(value.modified) ||
    !isUnsignedInteger(value.created) ||
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
    !onlyKeys(batch, SEARCH_BATCH_KEYS) ||
    !Array.isArray(batch.entries) ||
    typeof batch.done !== 'boolean' ||
    !isUnsignedInteger(batch.elapsedMs) ||
    typeof batch.truncated !== 'boolean' ||
    !isUnsignedInteger(batch.skipped) ||
    !isUnsignedInteger(batch.scanned) ||
    typeof batch.incomplete !== 'boolean' ||
    typeof batch.replace !== 'boolean' ||
    (batch.warning !== undefined && typeof batch.warning !== 'string') ||
    (batch.engine !== undefined && typeof batch.engine !== 'string')
  )
    throw new TypeError('Invalid search batch from IPC')
  const result: Batch = {
    entries: batch.entries.map(parseEntry),
    done: batch.done,
    elapsedMs: batch.elapsedMs,
    truncated: batch.truncated,
    skipped: batch.skipped,
    scanned: batch.scanned,
    incomplete: batch.incomplete,
    replace: batch.replace,
  }
  if (typeof batch.warning === 'string') result.warning = batch.warning
  if (typeof batch.engine === 'string') result.engine = batch.engine
  return result
}

export function parseListing(value: unknown): Listing {
  const listing = objectResult(value)
  if (
    !onlyKeys(listing, LISTING_KEYS) ||
    typeof listing.path !== 'string' ||
    !Array.isArray(listing.entries) ||
    !Array.isArray(listing.allowedOps) ||
    !listing.allowedOps.every((operation) => typeof operation === 'string') ||
    !Array.isArray(listing.warnings) ||
    !listing.warnings.every((warning) => typeof warning === 'string') ||
    typeof listing.virtualVolume !== 'boolean' ||
    (listing.defaultView !== undefined && !isListingView(listing.defaultView))
  )
    throw new TypeError('Invalid directory listing from IPC')
  return {
    path: listing.path,
    entries: listing.entries.map(parseEntry),
    allowedOps: listing.allowedOps,
    warnings: listing.warnings,
    virtualVolume: listing.virtualVolume,
    ...(isListingView(listing.defaultView) ? { defaultView: listing.defaultView } : {}),
  }
}

export function parseConnectionOpenResult(value: unknown): ConnectionOpenResult {
  const result = objectResult(value)
  if (
    result.kind === 'vfs' &&
    onlyKeys(result, ['kind', 'path']) &&
    typeof result.path === 'string'
  )
    return { kind: 'vfs', path: result.path }
  if (
    result.kind === 'pluginView' &&
    onlyKeys(result, ['kind', 'pluginId', 'connectionId', 'assetPath', 'state', 'tabTitle']) &&
    typeof result.pluginId === 'string' &&
    typeof result.connectionId === 'string' &&
    typeof result.assetPath === 'string' &&
    'state' in result &&
    (result.tabTitle === null || typeof result.tabTitle === 'string')
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
  const locations: Location[] = []
  for (const item of value) {
    if (
      !isJsonObject(item) ||
      !onlyKeys(item, [
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
      ]) ||
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
