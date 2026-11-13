import { describe, expect, it } from 'vitest'
import { parseInfoFieldDelta, parseInfoOpenResult, shouldApplyInfoDelta } from './ipcValidation'

describe('file info IPC validation', () => {
  it('only applies newer deltas for the active session', () => {
    expect(shouldApplyInfoDelta({ sessionId: 'active', revision: 3 }, 'active', 2)).toBe(true)
    expect(shouldApplyInfoDelta({ sessionId: 'active', revision: 2 }, 'active', 2)).toBe(false)
    expect(shouldApplyInfoDelta({ sessionId: 'old', revision: 9 }, 'active', 2)).toBe(false)
  })

  it('accepts one flat section per provider with provider-local subsection field keys', () => {
    const response = {
      sessionId: 'info-session',
      sections: [
        {
          id: 'core.metadata',
          title: '基本信息',
          providerId: 'core.metadata',
          provider: 'Fiex',
          source: 'core',
          fields: [
            {
              id: 'name',
              subsectionId: 'basic',
              label: '名称',
              compute: 'onOpen',
              status: 'pending',
            },
          ],
        },
        {
          id: 'plugin:org.example',
          title: 'Details',
          providerId: 'plugin:org.example',
          provider: 'Example',
          source: 'plugin',
          pluginId: 'org.example',
          fields: [
            {
              id: 'checksum',
              subsectionId: 'details',
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

  it('constructs Info DTOs and keeps optional values only when they are strings', () => {
    expect(
      parseInfoOpenResult({
        sessionId: 'info-session',
        sections: [
          {
            id: 'plugin:org.example',
            title: 'Details',
            providerId: 'plugin:org.example',
            provider: 'Example',
            source: 'plugin',
            pluginId: 'org.example',
            error: 'provider warning',
            fields: [
              {
                id: 'checksum',
                subsectionId: 'details',
                label: 'Checksum',
                compute: 'onDemand',
                status: 'ready',
                value: 'abc123',
              },
            ],
          },
        ],
      }),
    ).toEqual({
      sessionId: 'info-session',
      sections: [
        {
          id: 'plugin:org.example',
          title: 'Details',
          providerId: 'plugin:org.example',
          provider: 'Example',
          source: 'plugin',
          pluginId: 'org.example',
          error: 'provider warning',
          fields: [
            {
              id: 'checksum',
              subsectionId: 'details',
              label: 'Checksum',
              compute: 'onDemand',
              status: 'ready',
              value: 'abc123',
            },
          ],
        },
      ],
    })
  })

  it('rejects malformed file info sections before rendering them', () => {
    expect(() =>
      parseInfoOpenResult({
        sessionId: 'info-session',
        sections: [
          {
            id: 'core.metadata',
            title: '基本信息',
            providerId: 'core.metadata',
            provider: 'Fiex',
            source: 'core',
            fields: [
              {
                id: 'name',
                subsectionId: 'basic',
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

  it('rejects old section keys, duplicate providers, and duplicate field identities', () => {
    const base = {
      sessionId: 'info-session',
      sections: [
        {
          id: 'core.metadata',
          title: 'Metadata',
          providerId: 'core.metadata',
          provider: 'Fiex',
          source: 'core',
          fields: [
            {
              id: 'name',
              subsectionId: 'basic',
              label: 'Name',
              compute: 'onOpen',
              status: 'pending',
            },
          ],
        },
      ],
    }
    expect(() =>
      parseInfoOpenResult({
        ...base,
        sections: [
          { ...base.sections[0], fields: [{ ...base.sections[0].fields[0], sectionId: 'basic' }] },
        ],
      }),
    ).toThrow('Invalid file info response')
    expect(() =>
      parseInfoOpenResult({ ...base, sections: [...base.sections, ...base.sections] }),
    ).toThrow('Invalid file info response')
    expect(() =>
      parseInfoOpenResult({
        ...base,
        sections: [
          {
            ...base.sections[0],
            fields: [...base.sections[0].fields, ...base.sections[0].fields],
          },
        ],
      }),
    ).toThrow('Invalid file info response')
  })

  it('rejects unknown keys at the Info response envelope', () => {
    expect(() =>
      parseInfoOpenResult({ sessionId: 'info-session', sections: [], revision: 1 }),
    ).toThrow('Invalid file info response')
  })
})

describe('Info wire enum validation', () => {
  const field = {
    id: 'name',
    subsectionId: 'basic',
    label: 'Name',
    compute: 'onOpen',
    status: 'ready',
  }
  const section = {
    id: 'core.metadata',
    title: 'Metadata',
    providerId: 'core.metadata',
    provider: 'Fiex',
    source: 'core',
    fields: [field],
  }

  it.each([
    { ...section, source: ['core'] },
    { ...section, fields: [{ ...field, compute: ['onOpen'] }] },
    { ...section, fields: [{ ...field, status: ['ready'] }] },
  ])('rejects an enum encoded as an array in the Info response', (invalidSection) => {
    expect(() =>
      parseInfoOpenResult({ sessionId: 'info-session', sections: [invalidSection] }),
    ).toThrow('Invalid file info response')
  })

  it('rejects a field update whose status is an array', () => {
    expect(() =>
      parseInfoFieldDelta({
        sessionId: 'info-session',
        revision: 1,
        updates: [
          {
            providerId: section.providerId,
            subsectionId: field.subsectionId,
            fieldId: field.id,
            status: ['ready'],
          },
        ],
      }),
    ).toThrow('Invalid file info update')
  })

  it('constructs decoded Info field updates', () => {
    expect(
      parseInfoFieldDelta({
        sessionId: 'info-session',
        revision: 2,
        updates: [
          {
            providerId: 'core.metadata',
            subsectionId: 'basic',
            fieldId: 'name',
            status: 'ready',
            value: 'report.txt',
          },
        ],
      }),
    ).toEqual({
      sessionId: 'info-session',
      revision: 2,
      updates: [
        {
          providerId: 'core.metadata',
          subsectionId: 'basic',
          fieldId: 'name',
          status: 'ready',
          value: 'report.txt',
        },
      ],
    })
  })

  it('rejects unknown keys at the Info field delta envelope', () => {
    expect(() =>
      parseInfoFieldDelta({ sessionId: 'info-session', revision: 1, updates: [], stale: true }),
    ).toThrow('Invalid file info update')
  })
})
