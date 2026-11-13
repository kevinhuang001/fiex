import { describe, expect, it } from 'vitest'
import { parseEntryDecorations } from './decorations'

describe('decoration IPC decoding', () => {
  it('constructs decoration DTOs and normalizes nullable optional values', () => {
    expect(
      parseEntryDecorations([
        {
          path: '/workspace/report.txt',
          label: null,
          fields: {
            name: {
              badges: [{ text: 'PDF', color: 'red', tooltip: null }],
              style: {
                color: 'blue',
                fontWeight: null,
                fontStyle: 'italic',
                backgroundLight: null,
                backgroundDark: '#112233',
              },
            },
          },
          nativeBadges: [
            {
              text: 'Synced',
              color: 'green',
              tooltip: 'Remote file is synced',
              status: 'synced',
              icon: 'cloud-check',
              dot: false,
            },
          ],
          providers: ['core.remote'],
        },
      ]),
    ).toEqual([
      {
        path: '/workspace/report.txt',
        fields: {
          name: {
            badges: [{ text: 'PDF', color: 'red' }],
            style: { color: 'blue', fontStyle: 'italic', backgroundDark: '#112233' },
          },
        },
        nativeBadges: [
          {
            text: 'Synced',
            color: 'green',
            tooltip: 'Remote file is synced',
            status: 'synced',
            icon: 'cloud-check',
            dot: false,
          },
        ],
        providers: ['core.remote'],
      },
    ])
  })

  it('rejects malformed nested decoration fields and unknown targets', () => {
    expect(() => parseEntryDecorations([{ path: '/report.txt', extra: true }])).toThrow(
      'Invalid decoration',
    )
    expect(() =>
      parseEntryDecorations([{ path: '/report.txt', fields: { unknown: { badges: [] } } }]),
    ).toThrow('Invalid decoration target')
    expect(() =>
      parseEntryDecorations([
        {
          path: '/report.txt',
          fields: { name: { badges: [{ text: 'x', color: 'red', status: 'bad' }] } },
        },
      ]),
    ).toThrow('Invalid decoration badge')
  })
})
