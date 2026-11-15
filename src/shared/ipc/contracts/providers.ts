export interface ProviderEntry {
  id: string
  name: string
  pluginId?: string
  defaultPriority: number
  builtin: boolean
  locked?: boolean
  available?: boolean
  modes?: string[]
}

interface ProviderPreference {
  order: string[]
  disabled: string[]
}

export type ProviderPreferences = Record<string, ProviderPreference>
export type ProviderCatalog = Record<string, ProviderEntry[]>
export type Catalog = ProviderCatalog
