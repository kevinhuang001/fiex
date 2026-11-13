import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parseSearchProviders } from './searchProviders'

const provider = {
  id: 'plugin.org.example.search',
  provider: 'Example Search',
  priority: -10,
  builtin: false,
  modes: ['name', 'content'],
}

describe('parseSearchProviders', () => {
  it('constructs search engine DTOs and validates the command result', () => {
    expect(parseSearchProviders([provider])).toEqual([provider])
    expect(decodeDynamicIpcResult('search_providers', {}, [provider])).toEqual([provider])
  })

  it.each([
    null,
    {},
    { ...provider, id: '' },
    { ...provider, provider: null },
    { ...provider, priority: Number.MAX_SAFE_INTEGER + 1 },
    { ...provider, builtin: 'false' },
    { ...provider, modes: [] },
    { ...provider, modes: ['name', 'invalid'] },
    { ...provider, extra: true },
  ])('rejects malformed search provider entries', (value) => {
    expect(() => parseSearchProviders([value])).toThrow('Invalid search_providers IPC result')
  })
})
