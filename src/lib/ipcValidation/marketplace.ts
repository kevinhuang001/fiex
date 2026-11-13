import type { MarketplacePlugin, PluginAuthor } from '../plugins'
import { isJsonObject, onlyKeys, parseJsonValue } from './json'

const MARKETPLACE_PLUGIN_KEYS = [
  'id',
  'name',
  'version',
  'description',
  'author',
  'permissions',
  'requestedNetworkDomains',
  'commands',
  'extensions',
  'asset',
  'repository',
]

function parseStringList(value: unknown, command: string): string[] {
  if (!Array.isArray(value) || !value.every((item): item is string => typeof item === 'string'))
    throw new TypeError(`Invalid ${command} IPC result`)
  return value
}

function parseAuthor(value: unknown, command: string): PluginAuthor | null {
  if (value === null) return null
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['name', 'url']) ||
    typeof value.name !== 'string' ||
    !value.name ||
    !(value.url === null || typeof value.url === 'string')
  )
    throw new TypeError(`Invalid ${command} IPC result`)
  return { name: value.name, url: value.url }
}

function parseMarketplacePlugin(value: unknown): MarketplacePlugin {
  const command = 'plugin_marketplace'
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, MARKETPLACE_PLUGIN_KEYS) ||
    typeof value.id !== 'string' ||
    !value.id ||
    typeof value.name !== 'string' ||
    !value.name ||
    typeof value.version !== 'string' ||
    !value.version ||
    typeof value.description !== 'string' ||
    typeof value.asset !== 'string' ||
    !value.asset ||
    typeof value.repository !== 'string' ||
    !value.repository ||
    value.extensions === undefined
  )
    throw new TypeError(`Invalid ${command} IPC result`)

  return {
    id: value.id,
    name: value.name,
    version: value.version,
    description: value.description,
    author: parseAuthor(value.author, command),
    permissions: parseStringList(value.permissions, command),
    requestedNetworkDomains: parseStringList(value.requestedNetworkDomains, command),
    commands: parseStringList(value.commands, command),
    extensions: parseJsonValue(value.extensions),
    asset: value.asset,
    repository: value.repository,
  }
}

export function parseMarketplacePlugins(value: unknown): MarketplacePlugin[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid plugin_marketplace IPC result')
  return value.map(parseMarketplacePlugin)
}
