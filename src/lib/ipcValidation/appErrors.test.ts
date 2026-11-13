import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parsePendingErrors } from './appErrors'

const appError = {
  id: 1,
  at: 1_700_000_000_000,
  source: 'core.files',
  code: 'file.read_failed',
  message: 'Could not read the file',
  operation: null,
  retryable: false,
  occurrences: 1,
  lastAt: 1_700_000_000_000,
}

describe('pending application error IPC validation', () => {
  it('constructs the full AppError DTO including nullable operation', () => {
    expect(parsePendingErrors([appError])).toEqual([appError])
    expect(parsePendingErrors([{ ...appError, operation: 'read' }])).toEqual([
      { ...appError, operation: 'read' },
    ])
  })

  it.each([
    { ...appError, id: '1' },
    { ...appError, retryable: 0 },
    { ...appError, operation: undefined },
    { ...appError, occurrences: 1.5 },
    { ...appError, occurrences: 0x1_0000_0000 },
    { ...appError, extra: true },
  ])('rejects malformed application errors', (value) => {
    expect(() => parsePendingErrors([value])).toThrow('Invalid errors_pending IPC result')
  })

  it('rejects non-list responses and validates at dynamic dispatch', () => {
    expect(() => parsePendingErrors(appError)).toThrow('Invalid errors_pending IPC result')
    expect(decodeDynamicIpcResult('errors_pending', {}, [appError])).toEqual([appError])
    expect(() =>
      decodeDynamicIpcResult('errors_pending', {}, [{ ...appError, lastAt: -1 }]),
    ).toThrow('Invalid errors_pending IPC result')
  })
})
