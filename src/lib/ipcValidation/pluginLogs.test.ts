import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parsePluginLogPage } from './pluginLogs'

const record = {
  id: 12,
  at: 1_700_000_000_000,
  source: 'org.example.sync',
  level: 'warn',
  operationId: null,
  durationMs: null,
  message: 'Retry scheduled',
}

describe('plugin log page IPC validation', () => {
  it('constructs the Rust log page and record DTOs', () => {
    expect(parsePluginLogPage({ entries: [record], cursor: 12, hasMore: false })).toEqual({
      entries: [record],
      cursor: 12,
      hasMore: false,
    })
    expect(
      parsePluginLogPage({
        entries: [{ ...record, operationId: 'op-1', durationMs: 42, detail: 'network timeout' }],
        cursor: 12,
        hasMore: true,
      }),
    ).toEqual({
      entries: [{ ...record, operationId: 'op-1', durationMs: 42, detail: 'network timeout' }],
      cursor: 12,
      hasMore: true,
    })
  })

  it.each([
    { ...record, level: 'fatal' },
    { ...record, operationId: undefined },
    { ...record, durationMs: -1 },
    { ...record, detail: null },
    { ...record, extra: true },
  ])('rejects malformed log records', (invalidRecord) => {
    expect(() =>
      parsePluginLogPage({ entries: [invalidRecord], cursor: 12, hasMore: false }),
    ).toThrow('Invalid plugin_logs IPC result')
  })

  it('requires all page fields and decodes at the dynamic boundary', () => {
    expect(() => parsePluginLogPage({ entries: [], cursor: 0 })).toThrow(
      'Invalid plugin_logs IPC result',
    )
    expect(
      decodeDynamicIpcResult('plugin_logs', {}, { entries: [], cursor: 0, hasMore: false }),
    ).toEqual({
      entries: [],
      cursor: 0,
      hasMore: false,
    })
  })
})
