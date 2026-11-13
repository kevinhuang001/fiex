import type { PluginAuthor, PluginInfo, PluginExtensions } from '../plugins'
import { isJsonObject, isUnsignedInteger, onlyKeys, parseJsonValue } from './json'

const PLUGIN_INFO_KEYS = [
  'schemaVersion',
  'id',
  'name',
  'version',
  'description',
  'icon',
  'extensions',
  'uiTimeoutMs',
  'backendAvailable',
  'enabled',
  'author',
  'approved',
  'declaredPermissions',
  'grantedPermissions',
  'requestedNetworkAll',
  'requestedNetworkDomains',
  'allowedNetworkAll',
  'allowedNetworkDomains',
  'requestedCommands',
  'allowedCommands',
  'missingCommands',
]

function parsePluginAuthor(value: unknown): PluginAuthor | undefined {
  if (value === null) return undefined
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['name', 'url']) ||
    typeof value.name !== 'string' ||
    !(value.url === null || typeof value.url === 'string')
  )
    throw new TypeError('Invalid plugins_list IPC result')
  return { name: value.name, url: value.url }
}

function parseExtensions(value: unknown): PluginExtensions | undefined {
  const extensions = parseJsonValue(value)
  if (extensions === null) return undefined
  if (typeof extensions !== 'object' || Array.isArray(extensions))
    throw new TypeError('Invalid plugins_list IPC result')
  // Core emits extensions only from manifests that passed its schema validator.
  return extensions as PluginExtensions
}

function parsePluginInfo(value: unknown): PluginInfo {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, PLUGIN_INFO_KEYS) ||
    value.schemaVersion !== 2 ||
    typeof value.id !== 'string' ||
    !value.id ||
    typeof value.name !== 'string' ||
    !value.name ||
    typeof value.version !== 'string' ||
    !value.version ||
    typeof value.description !== 'string' ||
    !(value.icon === null || typeof value.icon === 'string') ||
    value.extensions === undefined ||
    !isUnsignedInteger(value.uiTimeoutMs) ||
    !Number.isSafeInteger(value.uiTimeoutMs) ||
    value.uiTimeoutMs < 1_000 ||
    value.uiTimeoutMs > 900_000 ||
    typeof value.backendAvailable !== 'boolean' ||
    typeof value.enabled !== 'boolean' ||
    typeof value.approved !== 'boolean' ||
    typeof value.requestedNetworkAll !== 'boolean' ||
    typeof value.allowedNetworkAll !== 'boolean'
  )
    throw new TypeError('Invalid plugins_list IPC result')

  const author = parsePluginAuthor(value.author)
  const extensions = parseExtensions(value.extensions)
  return {
    schemaVersion: 2,
    id: value.id,
    name: value.name,
    version: value.version,
    description: value.description,
    ...(typeof value.icon === 'string' ? { icon: value.icon } : {}),
    enabled: value.enabled,
    ...(author ? { author } : {}),
    approved: value.approved,
    declaredPermissions: parseStringList(value.declaredPermissions),
    grantedPermissions: parseStringList(value.grantedPermissions),
    requestedNetworkAll: value.requestedNetworkAll,
    requestedNetworkDomains: parseStringList(value.requestedNetworkDomains),
    allowedNetworkAll: value.allowedNetworkAll,
    allowedNetworkDomains: parseStringList(value.allowedNetworkDomains),
    requestedCommands: parseStringList(value.requestedCommands),
    allowedCommands: parseStringList(value.allowedCommands),
    missingCommands: parseStringList(value.missingCommands),
    backendAvailable: value.backendAvailable,
    uiTimeoutMs: value.uiTimeoutMs,
    ...(extensions ? { extensions } : {}),
  }
}

function parseStringList(value: unknown): string[] {
  if (!Array.isArray(value) || !value.every((item): item is string => typeof item === 'string'))
    throw new TypeError('Invalid plugins_list IPC result')
  return value
}

export function parsePluginCatalog(value: unknown): PluginInfo[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid plugins_list IPC result')
  return value.map(parsePluginInfo)
}
