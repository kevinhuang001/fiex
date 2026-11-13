import type { Theme } from '../themes'
import type { Catalog, ProviderPreferences } from '@/components/settings/ProviderSettings'
import type { InstallPreview } from '@/components/settings/installer/types'
import type { LocalPreview } from '@/components/settings/plugins/PluginAlerts'
import { isJsonObject, objectResult, parseJsonValue } from './json'

export function parseThemes(value: unknown): Theme[] {
  const json = parseJsonValue(value)
  if (!Array.isArray(json)) throw new TypeError('Invalid plugin theme list')
  for (const theme of json) {
    if (
      theme === null ||
      Array.isArray(theme) ||
      typeof theme !== 'object' ||
      typeof theme.id !== 'string' ||
      typeof theme.name !== 'string' ||
      !['light', 'dark'].includes(String(theme.mode)) ||
      !isJsonObject(theme.tokens) ||
      !Object.values(theme.tokens).every((token) => typeof token === 'string') ||
      (theme.darkTokens !== undefined &&
        (!isJsonObject(theme.darkTokens) ||
          !Object.values(theme.darkTokens).every((token) => typeof token === 'string')))
    )
      throw new TypeError('Invalid plugin theme')
  }
  return json as unknown as Theme[]
}

export function parseProviderCatalog(value: unknown): { features: Catalog } {
  const catalog = objectResult(value)
  const features = catalog.features
  if (features === null || Array.isArray(features) || typeof features !== 'object')
    throw new TypeError('Invalid provider catalog')
  for (const providers of Object.values(features)) {
    if (!Array.isArray(providers)) throw new TypeError('Invalid provider list')
    for (const provider of providers) {
      if (
        provider === null ||
        Array.isArray(provider) ||
        typeof provider !== 'object' ||
        typeof provider.id !== 'string' ||
        typeof provider.name !== 'string' ||
        typeof provider.defaultPriority !== 'number' ||
        typeof provider.builtin !== 'boolean' ||
        (provider.locked !== undefined && typeof provider.locked !== 'boolean') ||
        (provider.available !== undefined && typeof provider.available !== 'boolean') ||
        (provider.modes !== undefined &&
          (!Array.isArray(provider.modes) ||
            !provider.modes.every((mode) => typeof mode === 'string')))
      )
        throw new TypeError('Invalid provider catalog entry')
    }
  }
  return { features: features as unknown as Catalog }
}

export function parseProviderPreferences(value: unknown): ProviderPreferences {
  const preferences = objectResult(value)
  for (const preference of Object.values(preferences)) {
    if (
      preference === null ||
      Array.isArray(preference) ||
      typeof preference !== 'object' ||
      !Array.isArray(preference.order) ||
      !preference.order.every((id) => typeof id === 'string') ||
      !Array.isArray(preference.disabled) ||
      !preference.disabled.every((id) => typeof id === 'string')
    )
      throw new TypeError('Invalid provider preferences')
  }
  return preferences as unknown as ProviderPreferences
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
    (config.id !== null && typeof config.id !== 'string') ||
    typeof config.name !== 'string' ||
    typeof config.kind !== 'string' ||
    (config.pluginId !== null && typeof config.pluginId !== 'string') ||
    config.settings === null ||
    Array.isArray(config.settings) ||
    typeof config.settings !== 'object' ||
    !Object.values(config.settings).every(
      (setting) =>
        typeof setting === 'string' ||
        typeof setting === 'boolean' ||
        (typeof setting === 'number' && Number.isFinite(setting)),
    )
  )
    throw new TypeError('Invalid remote connection config')
  return config as unknown as ReturnType<typeof parseRemoteConnectionConfig>
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
