import type { JsonValue } from '../../../../sdk/ui/src/index'

export interface PluginField {
  id: string
  label: string
  /** `list` renders a row editor and stores a JSON array of strings. */
  type: 'text' | 'url' | 'number' | 'checkbox' | 'secret' | 'list' | 'file' | 'font' | 'color'
  placeholder?: string
  defaultValue?: string | boolean | string[]
  required?: boolean
  /** Add a browse button: file stores the path, fromFile loads text content. */
  fromFile?: boolean
  extensions?: string[]
}

interface PluginPanel {
  entry: string
  title?: string
  defaultSize?: { width: number; height: number }
}

export interface PluginContribution {
  title: string
  icon?: string
  panel?: PluginPanel
}

interface PluginLoadingPage {
  /** Package-relative entry validated by Core like a panel document. */
  entry: string
}

interface PluginPreviewContribution {
  match?: { extensions?: string[]; mimeTypes?: string[]; kinds?: string[] }
  priority?: number
  maxBytes?: number
  panel?: PluginPanel
  /** Optional document shown while `preview.open` is still running. */
  loading?: PluginLoadingPage
}

interface PluginInfoContribution {
  match?: { extensions?: string[]; mimeTypes?: string[]; kinds?: string[] }
  priority?: number
}

export interface MarketplacePlugin {
  id: string
  name: string
  version: string
  description: string
  author: PluginAuthor | null
  permissions: string[]
  requestedNetworkDomains: string[]
  commands: string[]
  extensions: JsonValue
  asset: string
  repository: string
}

export interface PluginAuthor {
  name: string
  url?: string | null
}

export interface PluginInfo {
  schemaVersion: number
  id: string
  name: string
  version: string
  description: string
  icon?: string
  enabled: boolean
  author?: PluginAuthor
  approved: boolean
  declaredPermissions: string[]
  grantedPermissions: string[]
  requestedNetworkAll: boolean
  requestedNetworkDomains: string[]
  allowedNetworkAll: boolean
  allowedNetworkDomains: string[]
  requestedCommands: string[]
  allowedCommands: string[]
  /** Requested commands that Core cannot resolve on PATH right now. */
  missingCommands: string[]
  backendAvailable: boolean
  /** Bounded `extensions.ui.timeoutMs` budget negotiated by Core. */
  uiTimeoutMs: number
  extensions?: PluginExtensions
}

export interface PluginExtensions {
  /** Explicit UI/backend request budget for slow providers. */
  ui?: { timeoutMs?: number }
  connections?: { title: string; fields: PluginField[]; description?: string; setupUrl?: string }
  toolbar?: PluginContribution
  contextMenus?: boolean
  /** Flat actions for a saved connection's sidebar menu. */
  connectionContextMenus?: boolean | { priority?: number }
  search?: { icon?: string; scope?: string }
  settings?: PluginField[]
  hooks?: {
    event: 'beforeOpen'
    priority?: number
    extensions?: string[]
  }
  preview?: PluginPreviewContribution
  info?: PluginInfoContribution
}

export interface ActivationConsent {
  permissions: string[]
  networkAll: boolean
  domains: string[]
  commands: string[]
}

export interface BeforeOpenOutcome {
  handled: boolean
}
