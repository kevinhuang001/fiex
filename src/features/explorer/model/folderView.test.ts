import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Preferences } from '@/lib/types'
import {
  folderViewPreferences,
  hasFolderOrderingChange,
  hasFolderViewChange,
  loadFolderView,
  patchFolderView,
  saveActiveFolderView,
  saveFolderView,
} from './folderView'

const memory = new Map<string, string>()
const storage = {
  getItem: (key: string) => memory.get(key) ?? null,
  setItem: (key: string, value: string) => memory.set(key, value),
}

describe('folderView', () => {
  afterEach(() => {
    memory.clear()
    vi.unstubAllGlobals()
  })

  it('uses a directory-specific presentation and provider view fallback', () => {
    vi.stubGlobal('localStorage', storage)
    const prefs = {
      view: 'list',
      sort: 'modified',
      descending: true,
      group: 'modified',
    } as Preferences
    saveFolderView('/tmp/one', {
      view: 'icons',
      sort: 'name',
      descending: false,
      group: 'kind',
    })

    expect(folderViewPreferences(prefs, '/tmp/one').view).toBe('icons')
    expect(folderViewPreferences(prefs, '/tmp/one').sort).toBe('name')
    expect(folderViewPreferences(prefs, '/tmp/two', 'gallery').view).toBe('gallery')
    expect(folderViewPreferences(prefs, '/tmp/two').sort).toBe('modified')
    expect(folderViewPreferences(prefs, '/tmp/two').descending).toBe(true)
    expect(folderViewPreferences(prefs, '/tmp/two').group).toBe('modified')
  })

  it('migrates the old default sort and grouping to modified descending', () => {
    vi.stubGlobal('localStorage', storage)
    storage.setItem(
      'fiex:folder-views',
      JSON.stringify({
        '/tmp/old': { view: 'list', sort: 'name', descending: false, group: 'none' },
      }),
    )

    expect(loadFolderView('/tmp/old')).toEqual({
      view: 'list',
      sort: 'modified',
      descending: true,
      group: 'modified',
    })
  })

  it('patches folder presentation fields without dropping the rest', () => {
    expect(
      patchFolderView(
        { view: 'list', sort: 'modified', descending: true, group: 'modified' },
        { sort: 'name' },
      ),
    ).toEqual({ view: 'list', sort: 'name', descending: true, group: 'modified' })
  })

  it('keeps unrelated preferences out of the directory presentation record', () => {
    const patch: Partial<Preferences> = { view: 'icons', hidden: true }

    expect(
      patchFolderView(
        { view: 'list', sort: 'modified', descending: true, group: 'modified' },
        patch,
      ),
    ).toEqual({ view: 'icons', sort: 'modified', descending: true, group: 'modified' })
  })

  it('classifies folder presentation and ordering preference patches', () => {
    expect(hasFolderViewChange({ view: 'icons' })).toBe(true)
    expect(hasFolderViewChange({ descending: false })).toBe(true)
    expect(hasFolderViewChange({})).toBe(false)

    expect(hasFolderOrderingChange({ sort: 'size' })).toBe(true)
    expect(hasFolderOrderingChange({ group: 'kind' })).toBe(true)
    expect(hasFolderOrderingChange({ view: 'columns' })).toBe(false)
    expect(hasFolderOrderingChange({})).toBe(false)
  })

  it('applies toolbar sorting only to the focused pane folder', () => {
    vi.stubGlobal('localStorage', storage)
    const left = {
      view: 'list',
      sort: 'name',
      descending: false,
      group: 'kind',
    } as const
    const right = {
      view: 'icons',
      sort: 'modified',
      descending: true,
      group: 'modified',
    } as const
    saveFolderView('/tmp/left', left)
    saveFolderView('/tmp/right', right)

    saveActiveFolderView(
      'right',
      {
        left: { path: '/tmp/left', preferences: left },
        right: { path: '/tmp/right', preferences: right },
      },
      { sort: 'size', descending: false },
    )

    expect(loadFolderView('/tmp/left')).toEqual(left)
    expect(loadFolderView('/tmp/right')).toEqual({ ...right, sort: 'size', descending: false })
  })
})
