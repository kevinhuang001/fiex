import { describe, expect, it } from 'vitest'
import { decodeDynamicIpcResult } from '../ipcValidation'
import { parseDropPlan } from './dropPlan'

const plan = {
  paths: ['/home/user/a.txt', '/home/user/b.txt'],
  moving: true,
  references: [
    { mountId: 'local', nodeId: 'node-a' },
    { mountId: 'local', nodeId: 'node-b' },
  ],
}

describe('parseDropPlan', () => {
  it('constructs a complete Core drop plan', () => {
    expect(parseDropPlan(plan)).toEqual(plan)
    expect(decodeDynamicIpcResult('internal_drop', {}, plan)).toEqual(plan)
  })

  it('accepts no captured internal drag session', () => {
    expect(parseDropPlan(null)).toBeNull()
    expect(decodeDynamicIpcResult('internal_drop', {}, null)).toBeNull()
  })

  it.each([
    {},
    { ...plan, paths: [] },
    { ...plan, paths: ['/home/user/a.txt'] },
    { ...plan, moving: 'yes' },
    {
      ...plan,
      references: [{ mountId: 'local', nodeId: 'node-a', extra: true }, plan.references[1]],
    },
    { ...plan, references: [{ mountId: '', nodeId: 'node-a' }, plan.references[1]] },
    { ...plan, references: [{ mountId: 'local', nodeId: 'node-a' }] },
    { ...plan, extra: true },
  ])('rejects malformed Core drop plans', (value) => {
    expect(() => parseDropPlan(value)).toThrow('Invalid internal_drop IPC result')
  })
})
