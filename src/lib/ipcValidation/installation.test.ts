import { describe, expect, it } from 'vitest'
import { parseInstallPreview } from './installation'

const preview = {
  ticket: 'ticket-1',
  id: 'org.example.plugin',
  name: 'Example',
  version: '1.2.3',
  description: 'Example plugin',
  icon: null,
  author: { name: 'Example Author', url: null },
  permissions: ['metadata'],
  requestedNetworkAll: false,
  requestedNetworkDomains: ['example.test'],
  commands: [],
  repository: 'https://example.test/plugin',
  signed: true,
  trusted: true,
  publisher: 'publisher-1',
  publisherName: null,
  keyId: 'key-1',
  publisherKey: null,
  fingerprint: 'fingerprint-1',
  isUpdate: false,
  previousVersion: null,
  permissionsChanged: false,
}

describe('plugin installation preview IPC validation', () => {
  it('constructs install preview DTOs and normalizes Rust null optionals', () => {
    expect(parseInstallPreview(preview)).toEqual({
      ticket: 'ticket-1',
      id: 'org.example.plugin',
      name: 'Example',
      version: '1.2.3',
      description: 'Example plugin',
      author: { name: 'Example Author' },
      permissions: ['metadata'],
      requestedNetworkAll: false,
      requestedNetworkDomains: ['example.test'],
      commands: [],
      repository: 'https://example.test/plugin',
      signed: true,
      trusted: true,
      publisher: 'publisher-1',
      keyId: 'key-1',
      fingerprint: 'fingerprint-1',
      isUpdate: false,
      permissionsChanged: false,
    })
  })

  it('projects local previews to the fields consumed by the local import dialog', () => {
    expect(parseInstallPreview(preview, true)).toEqual({
      ticket: 'ticket-1',
      id: 'org.example.plugin',
      name: 'Example',
      version: '1.2.3',
      author: { name: 'Example Author' },
      permissions: ['metadata'],
      requestedNetworkDomains: ['example.test'],
      commands: [],
      signed: true,
      trusted: true,
      publisher: 'publisher-1',
      keyId: 'key-1',
      fingerprint: 'fingerprint-1',
    })
  })

  it.each([
    { ...preview, unexpected: true },
    { ...preview, permissions: ['metadata', 1] },
    { ...preview, author: { name: 'Author', unexpected: true } },
    { ...preview, isUpdate: 'false' },
  ])('rejects malformed install previews', (value) => {
    expect(() => parseInstallPreview(value)).toThrow('Invalid plugin installation preview')
  })
})
