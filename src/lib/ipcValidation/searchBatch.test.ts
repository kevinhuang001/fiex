import { describe, expect, it } from 'vitest'
import { parseSearchBatch } from './entries'

const batch = {
  entries: [],
  done: false,
  elapsedMs: 3,
  truncated: false,
  skipped: 0,
  scanned: 4,
  incomplete: false,
  replace: false,
}
const missingScanned = {
  entries: [],
  done: false,
  elapsedMs: 3,
  truncated: false,
  skipped: 0,
  incomplete: false,
  replace: false,
}

describe('search batch IPC validation', () => {
  it('constructs the required Rust search batch DTO', () => {
    expect(parseSearchBatch(batch)).toEqual(batch)
  })

  it('preserves optional warning and engine values', () => {
    expect(
      parseSearchBatch({ ...batch, warning: 'Some items were skipped', engine: 'builtin' }),
    ).toEqual({
      ...batch,
      warning: 'Some items were skipped',
      engine: 'builtin',
    })
  })

  it.each([
    { ...batch, unexpected: true },
    missingScanned,
    { ...batch, elapsedMs: 1.5 },
    { ...batch, skipped: -1 },
    { ...batch, incomplete: 'false' },
    { ...batch, warning: null },
    { ...batch, engine: 4 },
    {
      entries: [],
      done: false,
      elapsedMs: 0,
      truncated: false,
      skipped: 0,
      scanned: 0,
      incomplete: false,
    },
  ])('rejects malformed search batch data: %j', (value) => {
    expect(() => parseSearchBatch(value)).toThrow('Invalid search batch from IPC')
  })
})
