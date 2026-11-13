import { describe, expect, it } from 'vitest'
import { parseOperationResult } from './entries'

const operationResult = {
  paths: ['/workspace/report.txt'],
  errors: [],
  skipped: 0,
  conflicts: [
    {
      key: 'conflict-key',
      source: { path: '/source/report.txt', size: 12, modified: 2, isDir: false },
      destination: { path: '/destination/report.txt', size: 16, modified: 3, isDir: false },
    },
  ],
}

describe('file operation result IPC validation', () => {
  it('constructs operation and nested conflict DTOs', () => {
    const decoded = parseOperationResult(operationResult)

    expect(decoded).toEqual(operationResult)
    expect(decoded).not.toBe(operationResult)
    expect(decoded.conflicts?.[0]).not.toBe(operationResult.conflicts[0])
  })

  it('accepts an empty Rust operation result', () => {
    expect(parseOperationResult({ paths: [], errors: [], skipped: 0, conflicts: [] })).toEqual({
      paths: [],
      errors: [],
      skipped: 0,
      conflicts: [],
    })
  })

  it.each([
    { ...operationResult, extra: true },
    { ...operationResult, paths: ['/ok', 1] },
    { ...operationResult, skipped: -1 },
    { ...operationResult, conflicts: [{ ...operationResult.conflicts[0], extra: true }] },
    {
      ...operationResult,
      conflicts: [
        {
          ...operationResult.conflicts[0],
          source: { ...operationResult.conflicts[0].source, size: 1.5 },
        },
      ],
    },
  ])('rejects malformed operation results: %j', (value) => {
    expect(() => parseOperationResult(value)).toThrow()
  })
})
