import { useEffect, useState } from 'react'
import { call, demo, native } from '@/lib/api'

export interface PluginField {
  id: string
  label: string
  type: 'text' | 'url' | 'number' | 'checkbox' | 'secret'
  placeholder?: string
  defaultValue?: string
  required?: boolean
}
export interface PluginContribution {
  id: string
  title: string
  icon?: string
  command?: string
  openPanel?: string
}
export interface MarketplacePlugin {
  id: string
  name: string
  version: string
  description: string
  author?: { name: string; url?: string }
  permissions: string[]
  requestedNetworkDomains: string[]
  asset: string
  repository: string
}
export interface PluginInfo {
  id: string
  name: string
  version: string
  description?: string
  capabilities: string[]
  enabled: boolean
  author?: { name: string; url?: string }
  approved: boolean
  declaredPermissions: string[]
  grantedPermissions: string[]
  requestedNetworkDomains: string[]
  allowedNetworkDomains: string[]
  backendAvailable: boolean
  contributes?: {
    connectionFactories?: Array<{ id: string; title: string; fields: PluginField[] }>
    toolbar?: PluginContribution[]
    panels?: Array<{ id: string; title: string; entry: string }>
    settings?: PluginField[]
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
