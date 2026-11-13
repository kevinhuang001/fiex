import { describe, expect, it } from 'vitest'
import { parseProviderCatalog, parseProviderPreferences, parseThemes } from './providers'

describe('provider catalog IPC validation', () => {
  it('constructs provider entries and retains declared plugin metadata', () => {
    expect(
      parseProviderCatalog({
        features: {
          search: [
            {
              id: 'core.search',
              name: 'Fiex',
              defaultPriority: 10,
              builtin: true,
              modes: ['name', 'content'],
            },
            {
              id: 'plugin:org.example',
              name: 'Example',
              pluginId: 'org.example',
              defaultPriority: 50,
              builtin: false,
              locked: false,
              available: true,
            },
          ],
        },
      }),
    ).toEqual({
      features: {
        search: [
          {
            id: 'core.search',
            name: 'Fiex',
            defaultPriority: 10,
            builtin: true,
            modes: ['name', 'content'],
          },
          {
            id: 'plugin:org.example',
            name: 'Example',
            pluginId: 'org.example',
            defaultPriority: 50,
            builtin: false,
            locked: false,
            available: true,
          },
        ],
      },
    })
  })

  it.each([
    { id: 'provider', name: 'Provider', defaultPriority: 1, builtin: true, extra: true },
    { id: 'provider', name: 'Provider', defaultPriority: '1', builtin: true },
    { id: 'provider', name: 'Provider', defaultPriority: 1, builtin: true, modes: ['name', 1] },
  ])('rejects malformed provider entries', (entry) => {
    expect(() => parseProviderCatalog({ features: { search: [entry] } })).toThrow(
      'Invalid provider catalog entry',
    )
  })

  it('rejects unknown catalog envelope keys', () => {
    expect(() => parseProviderCatalog({ features: {}, revision: 1 })).toThrow(
      'Invalid provider catalog',
    )
  })
})

describe('provider preferences IPC validation', () => {
  it('constructs feature preferences from string lists', () => {
    expect(
      parseProviderPreferences({
        search: { order: ['plugin:org.example', 'core.search'], disabled: ['plugin:other'] },
        preview: { order: [], disabled: [] },
      }),
    ).toEqual({
      search: { order: ['plugin:org.example', 'core.search'], disabled: ['plugin:other'] },
      preview: { order: [], disabled: [] },
    })
  })

  it.each([
    { search: { order: ['provider'], disabled: [], extra: true } },
    { search: { order: ['provider', 1], disabled: [] } },
    { search: { order: [], disabled: 'provider' } },
  ])('rejects malformed provider preferences', (preferences) => {
    expect(() => parseProviderPreferences(preferences)).toThrow('Invalid provider preferences')
  })
})

describe('theme IPC validation', () => {
  it('constructs theme DTOs and omits the absent dark variant', () => {
    expect(
      parseThemes([
        {
          id: 'org.example:light',
          name: 'Light',
          mode: 'light',
          tokens: { accent: '#0067c0' },
        },
        {
          id: 'org.example:dark',
          name: 'Dark',
          mode: 'dark',
          tokens: { accent: '#80bdff' },
          darkTokens: { canvas: '#111111' },
        },
      ]),
    ).toEqual([
      {
        id: 'org.example:light',
        name: 'Light',
        mode: 'light',
        tokens: { accent: '#0067c0' },
      },
      {
        id: 'org.example:dark',
        name: 'Dark',
        mode: 'dark',
        tokens: { accent: '#80bdff' },
        darkTokens: { canvas: '#111111' },
      },
    ])
  })

  it.each([
    { id: 'theme', name: 'Theme', mode: ['light'], tokens: {} },
    { id: 'theme', name: 'Theme', mode: 'light', tokens: { accent: 1 } },
    { id: 'theme', name: 'Theme', mode: 'light', tokens: {}, unexpected: true },
    { id: 'theme', name: 'Theme', mode: 'light', tokens: {}, darkTokens: null },
  ])('rejects malformed theme payloads', (theme) => {
    expect(() => parseThemes([theme])).toThrow('Invalid plugin theme')
  })

  it('rejects a non-array response', () => {
    expect(() => parseThemes({})).toThrow('Invalid plugin theme list')
  })
})
