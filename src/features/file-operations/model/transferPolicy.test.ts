import { describe, expect, it } from 'vitest'
import type { Entry } from '@/lib/types'
import { canDuplicateEntries, planTransferPaths } from './transferPolicy'

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
    expect(canDuplicateEntries([], ['mkdir', 'write'])).toBe(false)
    expect(canDuplicateEntries([undefined], ['mkdir', 'write'])).toBe(false)
    expect(canDuplicateEntries([entry('/docs/folder', true)], [])).toBe(false)
  })
})

describe('planTransferPaths', () => {
  it('removes moves already satisfied by the destination and tracks source parents', () => {
    expect(
      planTransferPaths(['/docs/report.txt', '/archive/old.txt'], '/docs', true, [
        undefined,
        '/archive',
      ]),
    ).toEqual({
      actionablePaths: ['/archive/old.txt'],
      affectedSourceDirectories: ['/docs', '/archive'],
    })
  })

  it('keeps copies actionable and does not mark source directories for refresh', () => {
    expect(planTransferPaths(['/docs/report.txt'], '/docs', false, [])).toEqual({
      actionablePaths: ['/docs/report.txt'],
      affectedSourceDirectories: [],
    })
  })
})
