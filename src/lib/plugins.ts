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
export interface PluginInfo {
  id: string
  name: string
  version: string
  capabilities: string[]
  enabled: boolean
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
    void pluginCatalog().then((plugins) => {
      if (mounted) setItems(plugins)
    }).catch(() => {})
    return () => { mounted = false }
  }, [])
  return items
}
