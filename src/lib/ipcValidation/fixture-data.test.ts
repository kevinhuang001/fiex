import { afterEach, expect, it, vi } from 'vitest'
import { parseEntry } from '@/lib/ipcValidation/entries'
import { serializeIpcJson } from '@/lib/ipcValidation/json'

afterEach(() => vi.unstubAllGlobals())

it('provides entries accepted by the Core IPC boundary', async () => {
  vi.stubGlobal('location', { search: '?count=100' })
  const { fixtureEntries, fixtureRemoteEntries } = await import('../../../e2e/fixtures/data')
  for (const entry of [...fixtureEntries, ...fixtureRemoteEntries]) {
    expect(() => parseEntry(serializeIpcJson(entry))).not.toThrow()
  }
})
