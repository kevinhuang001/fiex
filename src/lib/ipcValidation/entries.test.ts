import type { JsonValue } from '../../../sdk/ui/src/index'
import { describe, expect, it } from 'vitest'
import { parseEntry } from './entries'

const entry: JsonValue = {
  path: '/workspace/report.txt',
  name: 'report.txt',
  isDir: false,
  isSymlink: false,
  size: 1024,
  modified: 1700000000,
  created: 1690000000,
  extension: 'txt',
  mime: 'text/plain',
  linkTarget: null,
  allowedOps: ['stat', 'read', 'write'],
  tags: [{ name: 'Research', color: 3 }],
  hasCache: false,
  pinned: false,
  uploaded: false,
  sync: null,
}

describe('file entry IPC validation', () => {
  it('constructs an Entry DTO from the Rust file entry contract', () => {
    const decoded = parseEntry(entry)

    expect(decoded).toEqual(entry)
    expect(decoded).not.toBe(entry)
    expect(decoded.tags[0]).not.toBe((entry as { tags: JsonValue[] }).tags[0])
  })

  it('preserves nullable link and sync values and validates Finder tag colors', () => {
    expect(
      parseEntry({
        ...entry,
        linkTarget: '/workspace/target.txt',
        tags: [{ name: 'Personal', color: 7 }],
        sync: 'conflict',
      }),
    ).toMatchObject({
      linkTarget: '/workspace/target.txt',
      tags: [{ name: 'Personal', color: 7 }],
      sync: 'conflict',
    })
  })

  it.each([
    { ...entry, unexpected: true },
    { ...entry, size: -1 },
    { ...entry, modified: 1.5 },
    { ...entry, allowedOps: ['read', 1] },
    { ...entry, hasCache: undefined },
    { ...entry, sync: 'unknown' },
    { ...entry, tags: [{ name: 'Research', color: 8 }] },
    { ...entry, tags: [{ name: 'Research', color: 2, hex: '#123456' }] },
  ] as JsonValue[])('rejects malformed or unexpected entry data: %j', (value) => {
    expect(() => parseEntry(value)).toThrow()
  })
})
