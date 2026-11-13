import type { SystemProxyInfo } from '../types'
import { isJsonObject, onlyKeys } from './json'

function isProxySource(value: unknown): value is SystemProxyInfo['source'] {
  return value === 'windows-system' || value === 'macos-system' || value === 'environment'
}

export function parseSystemProxyInfo(value: unknown): SystemProxyInfo {
  if (
    !isJsonObject(value) ||
    !onlyKeys(value, ['enabled', 'source', 'proxy']) ||
    typeof value.enabled !== 'boolean' ||
    !isProxySource(value.source)
  )
    throw new TypeError('Invalid system_proxy_info IPC result')

  if (value.enabled) {
    if (typeof value.proxy !== 'string' || !value.proxy)
      throw new TypeError('Invalid system_proxy_info IPC result')
    return { enabled: true, source: value.source, proxy: value.proxy }
  }
  if (value.proxy !== null) throw new TypeError('Invalid system_proxy_info IPC result')
  return { enabled: false, source: value.source, proxy: null }
}
