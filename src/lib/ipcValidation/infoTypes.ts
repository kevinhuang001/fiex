export type InfoField = {
  id: string
  subsectionId: string
  label: string
  compute: 'onOpen' | 'onDemand'
  status: 'idle' | 'pending' | 'ready' | 'unavailable' | 'error'
  value?: string
  error?: string
}
export type InfoSection = {
  id: string
  title: string
  providerId: string
  provider: string
  source: 'core' | 'plugin'
  pluginId?: string
  fields: InfoField[]
  error?: string
}
export type InfoFieldUpdate = {
  providerId: string
  subsectionId: string
  fieldId: string
  status: InfoField['status']
  value?: string
  error?: string
}
export type InfoFieldDelta = { sessionId: string; revision: number; updates: InfoFieldUpdate[] }
export type InfoOpenResult = { sessionId: string; sections: InfoSection[] }
