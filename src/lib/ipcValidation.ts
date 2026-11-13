import { parseResourceContext } from '../../sdk/ui/src/resources.js'
import { objectResult, parseJsonValue } from './ipcValidation/json'
import {
  parseConnectionOpenResult,
  parseStateGetArgs,
  parseStateGetResult,
  parseListing,
  parseEntry,
  parseOperationResult,
  parseLocations,
} from './ipcValidation/entries'
import { parseInfoOpenResult, parseInfoFieldDelta } from './ipcValidation/info'
import { parseInstallPreview } from './ipcValidation/installation'
import { parseConnectionMenus } from './ipcValidation/menus'
import { parsePluginMenuResult, parseContextMenuChildItems } from './ipcValidation/contextMenus'
import { parseEntryDecorations } from './ipcValidation/decorations'
import {
  parseThemes,
  parseProviderCatalog,
  parseProviderPreferences,
  parseRemoteConnectionConfig,
} from './ipcValidation/providers'
import { parsePreviewCandidates, parseResolvedPreview } from './ipcValidation/previews'
import { decodeSimpleIpcResult } from './ipcValidation/simpleResults'
import { decodeSimpleCollectionIpcResult } from './ipcValidation/simpleCollections'
import { decodeRuntimeCatalogIpcResult } from './ipcValidation/runtimeCatalogs'
import { decodeRemoteConnectionIpcResult } from './ipcValidation/remoteConnections'
import { parsePendingErrors } from './ipcValidation/appErrors'

export * from './ipcValidation/json'
export * from './ipcValidation/entries'
export * from './ipcValidation/info'
export * from './ipcValidation/installation'
export * from './ipcValidation/directory'
export * from './ipcValidation/menus'
export * from './ipcValidation/contextMenus'
export * from './ipcValidation/decorations'
export * from './ipcValidation/providers'
export * from './ipcValidation/previews'

export function decodeDynamicIpcResult(
  command: string,
  args: Record<string, unknown>,
  value: unknown,
): unknown {
  if (command === 'connection_open') return parseConnectionOpenResult(value)
  if (command === 'state_get') return parseStateGetResult(parseStateGetArgs(args), value)
  if (command === 'list_directory') return parseListing(value)
  if (command === 'file_info') return parseEntry(parseJsonValue(value))
  if (command === 'file_info_open') return parseInfoOpenResult(value)
  if (command === 'file_info_compute') return parseInfoFieldDelta(value)
  if (command === 'transfer' || command === 'recycle') return parseOperationResult(value)
  if (command === 'locations') return parseLocations(parseJsonValue(value))
  if (command === 'plugin_context_menu_resolve') {
    const source = args.source
    if (source !== 'core' && source !== 'plugins')
      throw new TypeError('Missing context menu source')
    return source === 'core'
      ? parsePluginMenuResult(value, 'core')
      : parsePluginMenuResult(value, 'plugins')
  }
  if (command === 'plugin_context_menu_children') {
    const result = objectResult(value)
    if (Object.keys(result).some((key) => key !== 'items') || !Array.isArray(result.items))
      throw new TypeError('Invalid context submenu response')
    return { items: parseContextMenuChildItems(result.items) }
  }
  if (
    command === 'plugin_context_menu_execute' ||
    command === 'plugin_connection_menu_execute' ||
    command === 'plugin_invoke' ||
    command === 'plugin_task' ||
    command === 'plugin_storage_get'
  )
    return parseJsonValue(value)
  if (command === 'plugin_settings') return objectResult(value)
  if (command === 'provider_catalog') return parseProviderCatalog(value)
  if (
    command === 'provider_preferences' ||
    command === 'set_provider_preference' ||
    command === 'reset_provider_preference'
  )
    return parseProviderPreferences(value)
  if (command === 'plugin_prepare_install') return parseInstallPreview(value)
  if (command === 'plugin_prepare_local_install') return parseInstallPreview(value, true)
  if (command === 'plugin_connection_menu_resolve') return parseConnectionMenus(value)
  if (command === 'plugin_decorations') return parseEntryDecorations(value)
  if (command === 'plugin_themes') return parseThemes(value)
  if (command === 'preview_candidates') return parsePreviewCandidates(value)
  if (command === 'preview_open') return parseResolvedPreview(value)
  if (command === 'plugin_before_open') {
    const result = objectResult(value)
    if (typeof result.handled !== 'boolean') throw new TypeError('Invalid before-open result')
    return { handled: result.handled }
  }
  if (command === 'plugin_describe_resources') {
    if (!Array.isArray(value)) throw new TypeError('Invalid resource context list')
    return value.map(parseResourceContext)
  }
  if (command === 'plugin_toolbar_invoke') return parseJsonValue(value)
  if (command === 'remote_connection_config') return parseRemoteConnectionConfig(value)
  if (command === 'errors_pending') return parsePendingErrors(value)
  const simpleResult = decodeSimpleIpcResult(command, value)
  if (simpleResult) return simpleResult.value
  const simpleCollectionResult = decodeSimpleCollectionIpcResult(command, value)
  if (simpleCollectionResult) return simpleCollectionResult.value
  const runtimeCatalogResult = decodeRuntimeCatalogIpcResult(command, value)
  if (runtimeCatalogResult) return runtimeCatalogResult.value
  const remoteConnectionResult = decodeRemoteConnectionIpcResult(command, value)
  if (remoteConnectionResult) return remoteConnectionResult.value
  return value === undefined ? undefined : parseJsonValue(value)
}
