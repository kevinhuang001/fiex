export interface InstallPreview {
  ticket: string
  id: string
  name: string
  version: string
  description?: string
  icon?: string
  author?: { name: string; url?: string }
  permissions: string[]
  requestedNetworkAll: boolean
  requestedNetworkDomains: string[]
  commands: string[]
  repository: string
  signed: boolean
  trusted: boolean
  publisher?: string
  publisherName?: string
  keyId?: string
  publisherKey?: string
  fingerprint?: string
  isUpdate: boolean
  previousVersion?: string
  permissionsChanged: boolean
}
