import { describe, expect, it, vi } from 'vitest'
import type { DialogState } from '@/lib/dialogs'
import type { FileOpsContext } from './types'
import { recycleActions } from './recycle'

function createActions(run: FileOpsContext['run']) {
  const dialog: { current: DialogState | null } = { current: null }
  const setSelected = vi.fn()
  const actions = recycleActions({
    x: { path: '/Users/kevin/Documents', selected: ['draft.txt'], setSelected },
    inTrash: true,
    setDialog: (value: DialogState | null) => {
      dialog.current = value
    },
    run,
    activity: {
      progressChannel: vi.fn(),
      quietProgressChannel: vi.fn(),
      updateActivity: vi.fn(),
    },
  } as unknown as FileOpsContext)

  return { actions, dialog, setSelected }
}

describe('permanent deletion selection', () => {
  it('keeps the selection when the operation fails or is cancelled', async () => {
    const run = vi.fn(async () => undefined) as unknown as FileOpsContext['run']
    const { actions, dialog, setSelected } = createActions(run)

    actions.deletePermanently(['trash://draft.txt'])
    await dialog.current?.action('')

    expect(setSelected).not.toHaveBeenCalled()
  })

  it('clears the selection after the operation succeeds', async () => {
    const run = vi.fn(async () => 1) as unknown as FileOpsContext['run']
    const { actions, dialog, setSelected } = createActions(run)

    actions.deletePermanently(['trash://draft.txt'])
    await dialog.current?.action('')

    expect(setSelected).toHaveBeenCalledWith([])
  })
})
