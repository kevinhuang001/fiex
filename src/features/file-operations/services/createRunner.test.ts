import { describe, expect, it, vi } from 'vitest'
import { createRunner } from './createRunner'
import type { FileOperationRuntimeServices } from '../types'

function setup() {
  const setError = vi.fn()
  const setBusy = vi.fn()
  const reportPermissionError = vi.fn(() => false)
  const runtime: FileOperationRuntimeServices = {
    progressChannel: vi.fn(),
    quietProgressChannel: vi.fn(),
    updateActivity: vi.fn(),
    beginOperation: vi.fn(async () => {}),
    cancelOperation: vi.fn(async () => {}),
    beginActivity: vi.fn(() => 'activity-1'),
    finishActivity: vi.fn(),
    requestDirectoryRefresh: vi.fn(),
  }
  const runner = createRunner({
    x: { path: '/destination', selected: ['/source/item.txt'], setError },
    running: { current: false },
    setBusy: setBusy as never,
    runtime,
    reportPermissionError,
  })
  return { runner, runtime, setBusy, setError, reportPermissionError }
}

describe('createRunner', () => {
  it('serializes operations and clears the busy state after completion', async () => {
    const { runner, setBusy } = setup()
    let finish = () => {}
    const operation = new Promise<number>((resolve) => {
      finish = () => resolve(7)
    })
    const first = runner('Copying', () => operation)
    const second = await runner('Deleting', async () => 9)

    expect(second).toBeUndefined()
    finish()
    await expect(first).resolves.toBe(7)
    expect(setBusy.mock.calls).toEqual([['Copying'], ['']])
  })

  it('starts cancellable activity and refreshes the operation result scope', async () => {
    const { runner, runtime } = setup()
    const onActivity = vi.fn()
    const affectedDirectories = (result: { path: string } | undefined) =>
      result ? ['/destination'] : []

    await runner(
      'Copying',
      async () => ({ path: '/destination/item.txt' }),
      'item details',
      true,
      'transfer',
      onActivity,
      false,
      undefined,
      undefined,
      affectedDirectories,
    )

    expect(runtime.beginOperation).toHaveBeenCalledOnce()
    expect(runtime.beginActivity).toHaveBeenCalledWith(
      'Copying',
      'item details',
      expect.any(Function),
      'transfer',
      undefined,
      true,
    )
    expect(onActivity).toHaveBeenCalledWith('activity-1')
    expect(runtime.finishActivity).toHaveBeenCalledWith('activity-1')
    expect(runtime.requestDirectoryRefresh).toHaveBeenCalledWith(['/destination'])
  })

  it('reports failures with the selected path and reconciles known directories', async () => {
    const { runner, runtime, setError, reportPermissionError } = setup()
    const affectedDirectories = (result: string | undefined) =>
      result ? ['/destination'] : ['/source']

    await expect(
      runner(
        'Moving',
        async () => {
          throw new Error('permission denied')
        },
        undefined,
        false,
        'transfer',
        undefined,
        false,
        undefined,
        undefined,
        affectedDirectories,
      ),
    ).resolves.toBeUndefined()

    expect(reportPermissionError).toHaveBeenCalledWith(
      'Error: permission denied',
      'Moving\n/source/item.txt',
    )
    expect(setError).toHaveBeenNthCalledWith(1, '')
    expect(setError).toHaveBeenLastCalledWith('Error: permission denied')
    expect(runtime.requestDirectoryRefresh).toHaveBeenCalledWith(['/source'])
    expect(runtime.finishActivity).toHaveBeenCalledWith('activity-1', 'Error: permission denied')
  })
})
