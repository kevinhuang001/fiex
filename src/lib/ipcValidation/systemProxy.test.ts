import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parseSystemProxyInfo } from './systemProxy'

describe('parseSystemProxyInfo', () => {
  it('decodes enabled proxy sources and disabled system states', () => {
    expect(
      parseSystemProxyInfo({ enabled: true, source: 'environment', proxy: 'http://proxy:8080' }),
    ).toEqual({ enabled: true, source: 'environment', proxy: 'http://proxy:8080' })
    expect(parseSystemProxyInfo({ enabled: false, source: 'macos-system', proxy: null })).toEqual({
      enabled: false,
      source: 'macos-system',
      proxy: null,
    })
    expect(
      decodeDynamicIpcResult(
        'system_proxy_info',
        {},
        {
          enabled: true,
          source: 'windows-system',
          proxy: 'http://proxy:80',
        },
      ),
    ).toEqual({ enabled: true, source: 'windows-system', proxy: 'http://proxy:80' })
  })

  it.each([
    null,
    { enabled: 1, source: 'environment', proxy: null },
    { enabled: true, source: 'other', proxy: 'http://proxy:8080' },
    { enabled: true, source: 'environment', proxy: null },
    { enabled: true, source: 'environment', proxy: '' },
    { enabled: false, source: 'environment', proxy: 'http://proxy:8080' },
    { enabled: false, source: 'environment', proxy: undefined },
    { enabled: false, source: 'environment', proxy: null, extra: true },
  ])('rejects malformed proxy info', (value) => {
    expect(() => parseSystemProxyInfo(value)).toThrow('Invalid system_proxy_info IPC result')
  })
})
