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
}
export interface PluginInfoContribution {
  id: string
  title: string
  command: string
  extensions?: string[]
  priority?: number
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
  extensions?: PluginExtensions
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
  extensions?: PluginExtensions
}
export interface PluginExtensions {
  connections?: Array<{ id: string; title: string; fields: PluginField[] }>
  toolbar?: PluginContribution[]
  contextMenus?: boolean
  search?: Array<{ id: string; title: string; icon?: string }>
  settings?: PluginField[]
  hooks?: Array<{
    id: string
    event: 'beforeOpen'
    method: string
    priority?: number
    extensions?: string[]
  }>
  preview?: Array<{
    id: string
    title?: string
    extensions: string[]
    priority?: number
    maxBytes?: number
    panel: { entry: string; title?: string; defaultSize?: { width: number; height: number } }
  }>
  info?: PluginInfoContribution[]
}
export const PLUGIN_CATEGORIES = [
  { id: 'info', label: '简介' },
  { id: 'preview', label: '预览' },
  { id: 'contextMenus', label: '右键菜单' },
  { id: 'toolbar', label: '工具栏' },
  { id: 'search', label: '搜索' },
  { id: 'connections', label: '连接' },
  { id: 'hooks', label: '打开方式' },
  { id: 'themes', label: '主题' },
  { id: 'commands', label: '命令' },
] as const
export function pluginCategories(extensions?: PluginExtensions, commands: string[] = []): string[] {
  const present = PLUGIN_CATEGORIES.filter(({ id }) => {
    if (id === 'commands') return false
    const value = extensions?.[id as keyof PluginExtensions]
    return (Array.isArray(value) && value.length > 0) || value === true
  }).map(({ id }) => id)
  if (!present.length && commands.length) present.push('commands')
  return present
}
export function isNewerVersion(candidate: string, installed: string): boolean {
  const left = candidate.split('-')[0].split('.').map(Number)
  const right = installed.split('-')[0].split('.').map(Number)
  for (let index = 0; index < Math.max(left.length, right.length); index++) {
    const a = left[index] || 0
    const b = right[index] || 0
    if (a !== b) return a > b
  }
  return false
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
