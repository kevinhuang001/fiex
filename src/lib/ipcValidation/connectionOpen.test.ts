import { describe, expect, it } from 'vitest'
import { parseConnectionOpenResult } from './entries'

const pluginView = {
  kind: 'pluginView',
  pluginId: 'mail',
  connectionId: 'account-1',
  assetPath: '/plugins/mail/index.html',
  state: { folder: 'inbox' },
  tabTitle: null,
}

describe('connection navigation IPC validation', () => {
  it('constructs the VFS navigation variant and rejects extra fields', () => {
    expect(parseConnectionOpenResult({ kind: 'vfs', path: 'fiex://mount/home' })).toEqual({
      kind: 'vfs',
      path: 'fiex://mount/home',
    })
    expect(() =>
      parseConnectionOpenResult({ kind: 'vfs', path: 'fiex://mount/home', pluginId: 'mail' }),
    ).toThrow('Invalid connection navigation response')
  })

  it('normalizes Rust null tab titles and preserves JSON state', () => {
    expect(parseConnectionOpenResult(pluginView)).toEqual({
      kind: 'pluginView',
      pluginId: 'mail',
      connectionId: 'account-1',
      assetPath: '/plugins/mail/index.html',
      state: { folder: 'inbox' },
    })
    expect(parseConnectionOpenResult({ ...pluginView, tabTitle: 'Inbox' })).toMatchObject({
      tabTitle: 'Inbox',
    })
  })

  it.each([
    { ...pluginView, extra: true },
    { ...pluginView, tabTitle: 4 },
    {
      kind: 'pluginView',
      pluginId: 'mail',
      connectionId: 'account-1',
      assetPath: '/plugins/mail/index.html',
      state: { folder: 'inbox' },
    },
    {
      kind: 'pluginView',
      pluginId: 'mail',
      connectionId: 'account-1',
      assetPath: '/plugins/mail/index.html',
      tabTitle: null,
    },
  ])('rejects malformed plugin navigation results: %j', (value) => {
    expect(() => parseConnectionOpenResult(value)).toThrow('Invalid connection navigation response')
  })
})
