import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Preferences } from './types'
import {
  folderViewPreferences,
  loadFolderView,
  patchFolderView,
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
})
