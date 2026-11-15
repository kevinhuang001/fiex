import { describe, expect, it } from 'vitest'
import type { PreviewCandidate } from '@/lib/preview'
import { orderCandidates, pluginPreviewId } from './model'

const candidates: PreviewCandidate[] = [
  { id: 'builtin:image', provider: 'Image', priority: 1 },
  { id: 'plugin:z', provider: 'Z', priority: 4 },
  { id: 'plugin:a', provider: 'A', priority: 4 },
  { id: 'builtin:text', provider: 'Text', priority: 8 },
]

describe('preview model', () => {
  it('derives plugin identity only from plugin providers', () => {
    expect(pluginPreviewId('plugin:org.fiex.office')).toBe('org.fiex.office')
    expect(pluginPreviewId('builtin:text')).toBeNull()
    expect(pluginPreviewId('')).toBeNull()
    expect(pluginPreviewId(null)).toBeNull()
    expect(pluginPreviewId(undefined)).toBeNull()
  })

  it('applies saved order before priority and deterministic id tie-breakers', () => {
    expect(
      orderCandidates(candidates, ['builtin:image', 'plugin:z']).map((item) => item.id),
    ).toEqual(['builtin:image', 'plugin:z', 'builtin:text', 'plugin:a'])
    expect(orderCandidates(candidates, []).map((item) => item.id)).toEqual([
      'builtin:text',
      'plugin:a',
      'plugin:z',
      'builtin:image',
    ])
  })

  it('does not mutate the provider list', () => {
    const source = [...candidates]
    orderCandidates(source, ['plugin:z'])
    expect(source).toEqual(candidates)
  })
})
