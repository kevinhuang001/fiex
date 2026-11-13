import type { Theme } from '../themes'
import type {
  Catalog,
  ProviderEntry,
  ProviderPreferences,
} from '@/components/settings/ProviderSettings'
import type { InstallPreview } from '@/components/settings/installer/types'
import type { LocalPreview } from '@/components/settings/plugins/PluginAlerts'
import { isJsonObject, objectResult, parseJsonValue } from './json'

function onlyKeys(value: Record<string, unknown>, allowed: string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function stringArray(value: unknown): string[] | null {
  if (!Array.isArray(value)) return null
  const strings: string[] = []
  for (const item of value) {
    if (typeof item !== 'string') return null
    strings.push(item)
  }
  return strings
}

function parseProviderEntry(value: unknown): ProviderEntry | null {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, [
      'id',
      'name',
      'pluginId',
      'defaultPriority',
      'builtin',
      'locked',
      'available',
      'modes',
    ]) ||
    typeof value.id !== 'string' ||
    typeof value.name !== 'string' ||
    (value.pluginId !== undefined && typeof value.pluginId !== 'string') ||
    typeof value.defaultPriority !== 'number' ||
    !Number.isFinite(value.defaultPriority) ||
    typeof value.builtin !== 'boolean' ||
    (value.locked !== undefined && typeof value.locked !== 'boolean') ||
    (value.available !== undefined && typeof value.available !== 'boolean') ||
    (value.modes !== undefined && !stringArray(value.modes))
  )
    return null

  const modes = value.modes === undefined ? undefined : stringArray(value.modes)
  return {
    id: value.id,
    name: value.name,
    ...(typeof value.pluginId === 'string' ? { pluginId: value.pluginId } : {}),
    defaultPriority: value.defaultPriority,
    builtin: value.builtin,
    ...(typeof value.locked === 'boolean' ? { locked: value.locked } : {}),
    ...(typeof value.available === 'boolean' ? { available: value.available } : {}),
    ...(modes ? { modes } : {}),
  }
}

function parsePreference(value: unknown): { order: string[]; disabled: string[] } | null {
  if (!isJsonObject(value) || !onlyKeys(value, ['order', 'disabled'])) return null
  const order = stringArray(value.order)
  const disabled = stringArray(value.disabled)
  if (!order || !disabled) return null
  return { order, disabled }
}

function isThemeMode(value: unknown): value is Theme['mode'] {
  return value === 'light' || value === 'dark'
}

function parseThemeTokens(value: unknown): Record<string, string> | null {
  if (!isJsonObject(value)) return null
  const tokens: Record<string, string> = {}
  for (const [key, token] of Object.entries(value)) {
    if (typeof token !== 'string') return null
    tokens[key] = token
  }
  return tokens
}

export function parseThemes(value: unknown): Theme[] {
  const json = parseJsonValue(value)
  if (!Array.isArray(json)) throw new TypeError('Invalid plugin theme list')
  const themes: Theme[] = []
  for (const theme of json) {
    if (
      !isJsonObject(theme) ||
      Object.keys(theme).some(
        (key) => !['id', 'name', 'mode', 'tokens', 'darkTokens'].includes(key),
      ) ||
      typeof theme.id !== 'string' ||
      typeof theme.name !== 'string' ||
      !isThemeMode(theme.mode)
    )
      throw new TypeError('Invalid plugin theme')

    const tokens = parseThemeTokens(theme.tokens)
    const darkTokens =
      theme.darkTokens === undefined ? undefined : parseThemeTokens(theme.darkTokens)
    if (!tokens || (theme.darkTokens !== undefined && !darkTokens))
      throw new TypeError('Invalid plugin theme')
    themes.push({
      id: theme.id,
      name: theme.name,
      mode: theme.mode,
      tokens,
      ...(darkTokens ? { darkTokens } : {}),
    })
  }
  return themes
}

export function parseProviderCatalog(value: unknown): { features: Catalog } {
  const catalog = objectResult(value)
  const features = catalog.features
  if (!onlyKeys(catalog, ['features']) || !isJsonObject(features))
    throw new TypeError('Invalid provider catalog')
  const parsedFeatures: Array<[string, ProviderEntry[]]> = []
  for (const [feature, providers] of Object.entries(features)) {
    if (!Array.isArray(providers)) throw new TypeError('Invalid provider list')
    const parsedProviders: ProviderEntry[] = []
    for (const provider of providers) {
      const parsed = parseProviderEntry(provider)
      if (!parsed) throw new TypeError('Invalid provider catalog entry')
      parsedProviders.push(parsed)
    }
    parsedFeatures.push([feature, parsedProviders])
  }
  return { features: Object.fromEntries(parsedFeatures) }
}

export function parseProviderPreferences(value: unknown): ProviderPreferences {
  const preferences = objectResult(value)
  const entries: Array<[string, { order: string[]; disabled: string[] }]> = []
  for (const [feature, rawPreference] of Object.entries(preferences)) {
    const preference = parsePreference(rawPreference)
    if (!preference) throw new TypeError('Invalid provider preferences')
    entries.push([feature, preference])
  }
  return Object.fromEntries(entries)
}

export function parseRemoteConnectionConfig(value: unknown): {
  id: string | null
  name: string
  kind: string
  pluginId: string | null
  settings: Record<string, string | number | boolean>
} {
  const config = objectResult(value)
  if (
    !onlyKeys(config, ['id', 'name', 'kind', 'pluginId', 'settings']) ||
    (config.id !== null && typeof config.id !== 'string') ||
    typeof config.name !== 'string' ||
    typeof config.kind !== 'string' ||
    (config.pluginId !== null && typeof config.pluginId !== 'string') ||
    !isJsonObject(config.settings)
  )
    throw new TypeError('Invalid remote connection config')

  const settings: Array<[string, string | number | boolean]> = []
  for (const [key, setting] of Object.entries(config.settings)) {
    if (
      typeof setting !== 'string' &&
      typeof setting !== 'boolean' &&
      !(typeof setting === 'number' && Number.isFinite(setting))
    )
      throw new TypeError('Invalid remote connection config')
    settings.push([key, setting])
  }
  return {
    id: typeof config.id === 'string' ? config.id : null,
    name: config.name,
    kind: config.kind,
    pluginId: typeof config.pluginId === 'string' ? config.pluginId : null,
    settings: Object.fromEntries(settings),
  }
}

export function parseInstallPreview(value: unknown): InstallPreview
export function parseInstallPreview(value: unknown, local: true): LocalPreview
export function parseInstallPreview(
  value: unknown,
  local?: boolean,
): InstallPreview | LocalPreview {
  const preview = objectResult(value)
  if (
    typeof preview.ticket !== 'string' ||
    typeof preview.id !== 'string' ||
    typeof preview.name !== 'string' ||
    typeof preview.version !== 'string' ||
    !Array.isArray(preview.permissions) ||
    !preview.permissions.every((permission) => typeof permission === 'string') ||
    !Array.isArray(preview.requestedNetworkDomains) ||
    !preview.requestedNetworkDomains.every((domain) => typeof domain === 'string') ||
    !Array.isArray(preview.commands) ||
    !preview.commands.every((command) => typeof command === 'string') ||
    typeof preview.signed !== 'boolean' ||
    typeof preview.trusted !== 'boolean'
  )
    throw new TypeError('Invalid plugin installation preview')
  if (
    !local &&
    (typeof preview.requestedNetworkAll !== 'boolean' ||
      typeof preview.isUpdate !== 'boolean' ||
      typeof preview.permissionsChanged !== 'boolean' ||
      typeof preview.repository !== 'string')
  )
    throw new TypeError('Invalid plugin installation preview')
  return preview as unknown as InstallPreview | LocalPreview
}
