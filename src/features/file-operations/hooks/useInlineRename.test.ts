import type { Entry } from '@/lib/types'
import { describe, expect, it, vi } from 'vitest'
import { createInlineRenameActions, type InlineRenameOptions } from './useInlineRename'

const draft = { path: '/docs/draft.txt', allowedOps: ['read', 'rename'] } as Entry

function createContext(overrides: Partial<InlineRenameOptions> = {}) {
  const setEntries = vi.fn()
  const setSelected = vi.fn()
  const setRenaming = vi.fn()
  const setError = vi.fn()
  const refreshDirectory = vi.fn()
  const services: InlineRenameOptions['services'] = {
    renameEntry: vi.fn(async () => '/docs/renamed.txt'),
    renameUnsupportedMessage: () => 'Rename is unavailable here',
  }
  const options: InlineRenameOptions = {
    path: '/docs',
    entries: [draft],
    setEntries,
    setSelected,
    setRenaming,
    setError,
    refreshDirectory,
    services,
    ...overrides,
  }
  return {
    actions: createInlineRenameActions(options),
    options,
    services,
    setEntries,
    setSelected,
    setRenaming,
    setError,
    refreshDirectory,
  }
}

describe('inline rename actions', () => {
  it('selects a renameable entry and starts inline editing', () => {
    const { actions, setEntries, setSelected, setRenaming } = createContext()

    actions.beginRename(draft.path)

    expect(setEntries).not.toHaveBeenCalled()
    expect(setSelected).toHaveBeenCalledWith([draft.path])
    expect(setRenaming).toHaveBeenCalledWith(draft.path)
  })

  it('adds a newly created renameable entry before selecting it', () => {
    const createdEntry = { path: '/docs/new.txt', allowedOps: ['read', 'rename'] } as Entry
    const { actions, options, setEntries, setSelected, setRenaming } = createContext({
      entries: [],
    })

    actions.beginRename(createdEntry.path, createdEntry)

    const update = setEntries.mock.calls[0]?.[0] as (previous: Entry[]) => Entry[]
    expect(update(options.entries)).toEqual([createdEntry])
    expect(setSelected).toHaveBeenCalledWith([createdEntry.path])
    expect(setRenaming).toHaveBeenCalledWith(createdEntry.path)
  })

  it('ignores entries that cannot be renamed', () => {
    const blocked = { path: '/docs/locked.txt', allowedOps: ['read'] } as Entry
    const { actions, setEntries, setSelected, setRenaming } = createContext({ entries: [blocked] })

    actions.beginRename(blocked.path)

    expect(setEntries).not.toHaveBeenCalled()
    expect(setSelected).not.toHaveBeenCalled()
    expect(setRenaming).not.toHaveBeenCalled()
  })

  it('clears inline editing when cancelled', () => {
    const { actions, setRenaming } = createContext()

    actions.cancelRename()

    expect(setRenaming).toHaveBeenCalledWith(null)
  })

  it('reports unsupported operations without calling the provider', async () => {
    const { actions, options, services, setError, refreshDirectory } = createContext({
      entries: [{ ...draft, allowedOps: ['read'] } as Entry],
    })

    await expect(actions.renameTo(draft.path, 'renamed.txt')).resolves.toBe(false)

    expect(services.renameEntry).not.toHaveBeenCalled()
    expect(setError).toHaveBeenCalledWith('Rename is unavailable here')
    expect(refreshDirectory).not.toHaveBeenCalled()
    expect(options.setRenaming).not.toHaveBeenCalled()
  })

  it('keeps the editor open and reports provider failures', async () => {
    const { actions, services, setError, setRenaming, refreshDirectory } = createContext()
    vi.mocked(services.renameEntry).mockRejectedValueOnce(new Error('name is taken'))

    await expect(actions.renameTo(draft.path, 'taken.txt')).resolves.toBe(false)

    expect(setError).toHaveBeenCalledWith('Error: name is taken')
    expect(setRenaming).not.toHaveBeenCalled()
    expect(refreshDirectory).not.toHaveBeenCalled()
  })

  it('clears the editor, selects the returned path, and refreshes on success', async () => {
    const { actions, options, services, setError, setSelected, setRenaming, refreshDirectory } =
      createContext()
    vi.mocked(services.renameEntry).mockResolvedValueOnce('/docs/renamed.txt')

    await expect(actions.renameTo(draft.path, 'renamed.txt')).resolves.toBe(true)

    expect(services.renameEntry).toHaveBeenCalledWith(draft.path, 'renamed.txt')
    expect(setError).toHaveBeenCalledWith('')
    expect(setRenaming).toHaveBeenCalledWith(null)
    expect(setSelected).toHaveBeenCalledWith(['/docs/renamed.txt'])
    expect(refreshDirectory).toHaveBeenCalledWith([options.path])
  })
})
