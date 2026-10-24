export interface DemoPlugin {
  id: string
  name: string
  version: string
  description?: string
  enabled: boolean
  approved: boolean
  backendAvailable: boolean
  capabilities: string[]
  declaredPermissions: string[]
  grantedPermissions: string[]
  requestedNetworkDomains: string[]
  allowedNetworkDomains: string[]
  requestedCommands?: string[]
  allowedCommands?: string[]
  author?: { name: string; url?: string }
  contributions?: Record<string, unknown>
}
