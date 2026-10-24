import { describe, expect, it } from 'vitest'
import { operationPaths } from './operationPaths'

describe('operationPaths', () => {
  it('does not grant a file just because the picker returned it', () => {
    expect(operationPaths({ action: 'cancel' }, ['/tmp/one'])).toEqual([])
  })
  it('grants only files named by the later operation', () => {
    expect(operationPaths({ files: [{ path: '/tmp/two' }] }, ['/tmp/one', '/tmp/two'])).toEqual([
      '/tmp/two',
    ])
  })
  it('does not accept a sibling or prefix of the chosen file', () => {
    expect(operationPaths({ path: '/tmp/one-secret' }, ['/tmp/one'])).toEqual([])
  })
})
