import { describe, expect, it } from 'vitest'
import { parseThemes } from './providers'

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
