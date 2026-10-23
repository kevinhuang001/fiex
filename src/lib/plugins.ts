import { useEffect, useState } from 'react'
import { call, demo, native } from '@/lib/api'

export interface PluginField {
  id: string
  label: string
  /** `list` renders a row editor and stores a JSON array of strings. */
  type: 'text' | 'url' | 'number' | 'checkbox' | 'secret' | 'list'
  placeholder?: string
  defaultValue?: string | boolean | string[]
  required?: boolean
}
export interface PluginPanel {
  entry: string
  title?: string
  defaultSize?: { width: number; height: number }
}
export interface PluginContribution {
  id: string
  title: string
  icon?: string
  command?: string
  panel?: PluginPanel
  children?: PluginContribution[]
}
export interface MarketplacePlugin {
  id: string
  name: string
  version: string
  description: string
  icon?: string
  author?: { name: string; url?: string }
  permissions: string[]
  requestedNetworkDomains: string[]
  commands: string[]
  asset: string
  repository: string
}
export interface PluginInfo {
  schemaVersion: number
  id: string
  name: string
  version: string
  description?: string
  icon?: string
  capabilities: string[]
  enabled: boolean
  author?: { name: string; url?: string }
  approved: boolean
  declaredPermissions: string[]
  grantedPermissions: string[]
  requestedNetworkDomains: string[]
  allowedNetworkDomains: string[]
  requestedCommands: string[]
  allowedCommands: string[]
  /** Requested commands that Core cannot resolve on PATH right now. */
  missingCommands: string[]
  backendAvailable: boolean
  contributions?: {
    connections?: Array<{ id: string; title: string; fields: PluginField[] }>
    toolbar?: PluginContribution[]
    contextMenus?: PluginContribution[]
    search?: Array<{ id: string; title: string; icon?: string }>
    settings?: PluginField[]
    preview?: Array<{
      id: string
      title?: string
      extensions: string[]
      priority?: number
      maxBytes?: number
      panel: { entry: string; title?: string; defaultSize?: { width: number; height: number } }
    }>
  }
}
export async function pluginCatalog(): Promise<PluginInfo[]> {
  if (!native && !demo) return []
  return call<PluginInfo[]>('plugins_list')
}
export function usePluginCatalog(): PluginInfo[] {
  const [items, setItems] = useState<PluginInfo[]>([])
  useEffect(() => {
    let mounted = true
    const refresh = () =>
      void pluginCatalog()
        .then((plugins) => {
          if (mounted) setItems(plugins)
        })
        .catch(() => {})
    refresh()
    window.addEventListener('fiex:plugins-changed', refresh)
    return () => {
      mounted = false
      window.removeEventListener('fiex:plugins-changed', refresh)
    }
  }, [])
  return items
}

export function refreshPluginCatalog() {
  window.dispatchEvent(new Event('fiex:plugins-changed'))
}

export async function pluginMarketplace(): Promise<MarketplacePlugin[]> {
  return call<MarketplacePlugin[]>('plugin_marketplace', {
    repository: 'kevinhuang001/fiex-official-plugins',
  })
}
