import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parseRecentNavigationTarget } from './recentNavigation'

describe('parseRecentNavigationTarget', () => {
  it('constructs the parent and target DTO', () => {
    const result = { parent: 'recent://documents', target: '/home/user/report.pdf' }
    expect(parseRecentNavigationTarget(result)).toEqual(result)
    expect(decodeDynamicIpcResult('recent_navigation_target', {}, result)).toEqual(result)
  })

  it.each([
    null,
    [],
    { target: '/home/user/report.pdf' },
    { parent: '', target: '/home/user/report.pdf' },
    { parent: 'recent://documents', target: '' },
    { parent: 'recent://documents', target: 42 },
    { parent: 'recent://documents', target: '/home/user/report.pdf', extra: true },
  ])('rejects malformed navigation results', (value) => {
    expect(() => parseRecentNavigationTarget(value)).toThrow(
      'Invalid recent_navigation_target IPC result',
    )
  })
})
