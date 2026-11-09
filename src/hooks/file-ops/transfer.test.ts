import { describe, expect, it } from 'vitest'
import type { Entry } from '@/lib/types'
import { canDuplicateEntries } from './transfer'

const entry = (path: string, isDir: boolean) => ({ path, isDir }) as Entry

describe('canDuplicateEntries', () => {
  it('checks file writes and directory creation separately', () => {
    expect(canDuplicateEntries([entry('/docs/report.txt', false)], ['write'])).toBe(true)
    expect(canDuplicateEntries([entry('/docs/folder', true)], ['mkdir'])).toBe(true)
    expect(canDuplicateEntries([entry('/docs/folder', true)], ['write'])).toBe(false)
    expect(
      canDuplicateEntries(
        [entry('/docs/folder', true), entry('/docs/report.txt', false)],
        ['mkdir', 'write'],
      ),
    ).toBe(true)
  })

  it('rejects missing metadata and unsupported destination operations', () => {
    expect(canDuplicateEntries([undefined], ['mkdir', 'write'])).toBe(false)
    expect(canDuplicateEntries([entry('/docs/folder', true)], [])).toBe(false)
  })
})
