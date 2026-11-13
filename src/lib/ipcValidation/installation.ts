import type { InstallPreview } from '@/components/settings/installer/types'
import type { LocalPreview } from '@/components/settings/plugins/PluginAlerts'
import { isJsonObject, objectResult } from './json'

const INVALID_PREVIEW = 'Invalid plugin installation preview'
const PREVIEW_KEYS = [
  'ticket',
  'id',
  'name',
  'version',
  'description',
  'icon',
  'author',
  'permissions',
  'requestedNetworkAll',
  'requestedNetworkDomains',
  'commands',
  'repository',
  'signed',
  'trusted',
  'publisher',
  'publisherName',
  'keyId',
  'publisherKey',
  'fingerprint',
  'isUpdate',
  'previousVersion',
  'permissionsChanged',
]

function onlyKeys(value: Record<string, unknown>, allowed: string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function stringList(value: unknown): string[] | null {
  if (!Array.isArray(value)) return null
  const result: string[] = []
  for (const item of value) {
    if (typeof item !== 'string') return null
    result.push(item)
  }
  return result
}

function optionalString(value: Record<string, unknown>, key: string): string | undefined {
  const entry = value[key]
  if (entry === undefined || entry === null) return undefined
  if (typeof entry !== 'string') throw new TypeError(INVALID_PREVIEW)
  return entry
}

function optionalAuthor(value: unknown): InstallPreview['author'] {
  if (value === undefined || value === null) return undefined
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['name', 'url']) ||
    typeof value.name !== 'string' ||
    (value.url !== undefined && value.url !== null && typeof value.url !== 'string')
  )
    throw new TypeError(INVALID_PREVIEW)
  return {
    name: value.name,
    ...(typeof value.url === 'string' ? { url: value.url } : {}),
  }
}

function decodeInstallPreview(value: unknown): InstallPreview {
  const preview = objectResult(value)
  const permissions = stringList(preview.permissions)
  const requestedNetworkDomains = stringList(preview.requestedNetworkDomains)
  const commands = stringList(preview.commands)
  if (
    !onlyKeys(preview, PREVIEW_KEYS) ||
    typeof preview.ticket !== 'string' ||
    typeof preview.id !== 'string' ||
    typeof preview.name !== 'string' ||
    typeof preview.version !== 'string' ||
    typeof preview.description !== 'string' ||
    (preview.icon !== undefined && preview.icon !== null && typeof preview.icon !== 'string') ||
    !permissions ||
    typeof preview.requestedNetworkAll !== 'boolean' ||
    !requestedNetworkDomains ||
    !commands ||
    typeof preview.repository !== 'string' ||
    typeof preview.signed !== 'boolean' ||
    typeof preview.trusted !== 'boolean' ||
    typeof preview.isUpdate !== 'boolean' ||
    typeof preview.permissionsChanged !== 'boolean'
  )
    throw new TypeError(INVALID_PREVIEW)

  const author = optionalAuthor(preview.author)
  const icon = optionalString(preview, 'icon')
  const publisher = optionalString(preview, 'publisher')
  const publisherName = optionalString(preview, 'publisherName')
  const keyId = optionalString(preview, 'keyId')
  const publisherKey = optionalString(preview, 'publisherKey')
  const fingerprint = optionalString(preview, 'fingerprint')
  const previousVersion = optionalString(preview, 'previousVersion')
  return {
    ticket: preview.ticket,
    id: preview.id,
    name: preview.name,
    version: preview.version,
    description: preview.description,
    ...(icon !== undefined ? { icon } : {}),
    ...(author ? { author } : {}),
    permissions,
    requestedNetworkAll: preview.requestedNetworkAll,
    requestedNetworkDomains,
    commands,
    repository: preview.repository,
    signed: preview.signed,
    trusted: preview.trusted,
    ...(publisher !== undefined ? { publisher } : {}),
    ...(publisherName !== undefined ? { publisherName } : {}),
    ...(keyId !== undefined ? { keyId } : {}),
    ...(publisherKey !== undefined ? { publisherKey } : {}),
    ...(fingerprint !== undefined ? { fingerprint } : {}),
    isUpdate: preview.isUpdate,
    ...(previousVersion !== undefined ? { previousVersion } : {}),
    permissionsChanged: preview.permissionsChanged,
  }
}

export function parseInstallPreview(value: unknown): InstallPreview
export function parseInstallPreview(value: unknown, local: true): LocalPreview
export function parseInstallPreview(
  value: unknown,
  local?: boolean,
): InstallPreview | LocalPreview {
  const preview = decodeInstallPreview(value)
  if (!local) return preview
  return {
    ticket: preview.ticket,
    id: preview.id,
    name: preview.name,
    version: preview.version,
    ...(preview.author ? { author: { name: preview.author.name } } : {}),
    permissions: preview.permissions,
    requestedNetworkDomains: preview.requestedNetworkDomains,
    commands: preview.commands,
    signed: preview.signed,
    trusted: preview.trusted,
    ...(preview.publisher ? { publisher: preview.publisher } : {}),
    ...(preview.publisherName ? { publisherName: preview.publisherName } : {}),
    ...(preview.keyId ? { keyId: preview.keyId } : {}),
    ...(preview.fingerprint ? { fingerprint: preview.fingerprint } : {}),
  }
}
