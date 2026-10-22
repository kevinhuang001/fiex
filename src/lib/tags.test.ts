import { describe, expect, it } from 'vitest'
import { normalizeTags } from './tags'

describe('tag catalogue normalization', () => {
  it('keeps valid tags and normalizes colours', () => {
    expect(
      normalizeTags([
        { name: '工作', color: 4, hex: '#007AFF' },
        { name: ' 归档 ', color: 7 },
      ]),
    ).toEqual([
      { name: '工作', color: 4, hex: '#007aff' },
      { name: '归档', color: 7 },
    ])
  })

  it('drops invalid entries and duplicate names', () => {
    expect(
      normalizeTags([
        { name: '工作', color: 4 },
        { name: '工作', color: 2 },
        { name: '', color: 2 },
        { name: '坏颜色', color: 0 },
        { name: '坏颜色', color: 8 },
        { name: '坏颜色', color: 2.5 },
        null,
        'nope',
      ]),
    ).toEqual([{ name: '工作', color: 4 }])
  })

  it('ignores malformed hex values', () => {
    expect(normalizeTags([{ name: '工作', color: 4, hex: 'red' }])).toEqual([
      { name: '工作', color: 4 },
    ])
  })

  it('returns null when the stored value is not a list', () => {
    expect(normalizeTags(undefined)).toBe(null)
    expect(normalizeTags({})).toBe(null)
    expect(normalizeTags([])).toEqual([])
  })
})
