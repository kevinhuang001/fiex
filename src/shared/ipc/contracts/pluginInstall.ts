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

/** Preview returned by the host before a local `.fiexplugin` file is confirmed. */
export interface LocalPreview {
  ticket: string
  id: string
  name: string
  version: string
  author?: { name: string }
  permissions: string[]
  requestedNetworkDomains: string[]
  commands: string[]
  signed: boolean
  trusted: boolean
  publisher?: string
  publisherName?: string
  keyId?: string
  fingerprint?: string
}
