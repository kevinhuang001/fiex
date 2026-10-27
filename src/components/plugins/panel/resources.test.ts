import { describe, expect, it, vi } from 'vitest'
const { call } = vi.hoisted(() => ({
  call: vi.fn(async (_command, { paths }) =>
    paths.map((path: string, index: number) => ({
      ref: { mountId: 'local', nodeId: path + ':' + index },
      metadata: { name: 'picked', kind: 'file' },
    })),
  ),
}))
vi.mock('@/lib/api', () => ({ call }))
import { PanelResources } from './resources'

describe('panel candidate resource refs', () => {
  it('describing a picker result grants nothing until the later invocation names its ref', async () => {
    const resources = new PanelResources('test')
    const [picked] = await resources.describe(['/tmp/picked'])
    expect(resources.bindings({ cancelled: true })).toEqual([])
    expect(resources.bindings({ input: picked.ref })).toEqual([
      { ref: picked.ref, path: '/tmp/picked' },
    ])
  })
  it('does not accept raw path strings or forged refs as file grants', async () => {
    const resources = new PanelResources('test')
    await resources.describe(['/tmp/picked'])
    expect(resources.bindings({ path: '/tmp/picked' })).toEqual([])
    expect(resources.bindings({ input: { mountId: 'local', nodeId: 'forged' } })).toEqual([])
  })
  it('only grants a directory when that candidate ref is submitted', async () => {
    const resources = new PanelResources('test')
    const [input, directory] = await resources.describe(['/tmp/picked', '/tmp'])
    expect(resources.bindings({ input: input.ref }).map((item) => item.path)).toEqual([
      '/tmp/picked',
    ])
    expect(
      resources.bindings({ input: input.ref, output: directory.ref }).map((item) => item.path),
    ).toEqual(['/tmp/picked', '/tmp'])
  })
  it('reuses identities within a panel and isolates different panels', async () => {
    const one = new PanelResources('test')
    const [picked] = await one.describe(['/tmp/picked'])
    expect((await one.describe(['/tmp/picked']))[0]).toBe(picked)
    expect(new PanelResources('test').bindings({ input: picked.ref })).toEqual([])
  })
})
