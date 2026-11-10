import { describe, expect, it } from 'vitest'
import { parseInfoOpenResult } from './ipcValidation'

describe('file info IPC validation', () => {
  it('accepts provider-local field section IDs in globally scoped Info sections', () => {
    const response = {
      sessionId: 'info-session',
      sections: [
        {
          id: 'core.metadata::basic',
          title: '基本信息',
          providerId: 'core.metadata',
          provider: 'Fiex',
          source: 'core',
          fields: [
            {
              id: 'name',
              sectionId: 'basic',
              label: '名称',
              compute: 'onOpen',
              status: 'pending',
            },
          ],
        },
        {
          id: 'plugin:org.example::details',
          title: 'Details',
          providerId: 'plugin:org.example',
          provider: 'Example',
          source: 'plugin',
          pluginId: 'org.example',
          fields: [
            {
              id: 'checksum',
              sectionId: 'details',
              label: 'Checksum',
              compute: 'onDemand',
              status: 'idle',
            },
          ],
        },
      ],
    }
    expect(parseInfoOpenResult(response)).toEqual(response)
  })

  it('rejects malformed file info sections before rendering them', () => {
    expect(() =>
      parseInfoOpenResult({
        sessionId: 'info-session',
        sections: [
          {
            id: 'core.metadata::basic',
            title: '基本信息',
            providerId: 'core.metadata',
            provider: 'Fiex',
            source: 'core',
            fields: [
              {
                id: 'name',
                sectionId: 'basic',
                label: '名称',
                compute: 'onOpen',
                status: 'invalid-status',
              },
            ],
          },
        ],
      }),
    ).toThrow('Invalid file info response')
  })
})
