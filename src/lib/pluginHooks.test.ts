import { describe, expect, it, vi } from 'vitest'
import { call } from './api'
import { runBeforeOpenHooks } from './pluginHooks'
import type { Entry } from './types'

vi.mock('./api', () => ({ call: vi.fn() }))
const invoke = vi.mocked(call)
const entry = { name: 'b.7z', path: '/a/b.7z', extension: '7z', isDir: false } as Entry

describe('before-open event router', () => {
  it('passes only the resource path to the Core-owned hook entrypoint', async () => {
    invoke.mockResolvedValueOnce({ handled: true, preview: true })
    await expect(runBeforeOpenHooks(entry)).resolves.toEqual({ handled: true, preview: true })
    expect(invoke).toHaveBeenCalledWith('plugin_before_open', { path: '/a/b.7z' })
  })

  it('does not block opening when the event router or a provider fails', async () => {
    invoke.mockRejectedValueOnce(new Error('provider failed'))
    await expect(runBeforeOpenHooks(entry)).resolves.toEqual({ handled: false })
  })
})
