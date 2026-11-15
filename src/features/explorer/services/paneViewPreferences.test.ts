import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Preferences } from '@/lib/types'
import { createPaneViewPreferences } from './paneViewPreferences'
import { loadFolderView, saveFolderView } from '../model/folderView'

const values = new Map<string, string>()
const storage = {
  getItem: (key: string) => values.get(key) ?? null,
  setItem: (key: string, value: string) => values.set(key, value),
}

const prefs = {
  view: 'list',
  sort: 'modified',
  descending: true,
  group: 'modified',
} as Preferences

const leftView = { view: 'list', sort: 'name', descending: false, group: 'kind' } as const
const rightView = { view: 'icons', sort: 'modified', descending: true, group: 'modified' } as const

describe('createPaneViewPreferences', () => {
  afterEach(() => {
    values.clear()
    vi.unstubAllGlobals()
  })

  it('saves the changed pane and preserves the other pane before changing defaults', () => {
    vi.stubGlobal('localStorage', storage)
    saveFolderView('/left', leftView)
    saveFolderView('/right', rightView)
    const update = vi.fn()
    const controller = createPaneViewPreferences({
      prefs,
      left: { path: '/left', defaultView: null },
      right: { path: '/right', defaultView: null },
      active: 'left',
      update,
    })

    controller.updateLeft({ view: 'columns' })

    expect(loadFolderView('/left')).toEqual({ ...leftView, view: 'columns' })
    expect(loadFolderView('/right')).toEqual(rightView)
    expect(update).toHaveBeenCalledWith({ view: 'columns' })
  })

  it('persists toolbar sorting to the active pane only', () => {
    vi.stubGlobal('localStorage', storage)
    saveFolderView('/left', leftView)
    saveFolderView('/right', rightView)
    const update = vi.fn()
    const controller = createPaneViewPreferences({
      prefs,
      left: { path: '/left', defaultView: null },
      right: { path: '/right', defaultView: null },
      active: 'right',
      update,
    })

    controller.updateToolbar({ sort: 'size', descending: false })

    expect(loadFolderView('/left')).toEqual(leftView)
    expect(loadFolderView('/right')).toEqual({ ...rightView, sort: 'size', descending: false })
    expect(update).toHaveBeenCalledWith({ sort: 'size', descending: false })
  })

  it('does not persist non-view preferences per directory', () => {
    vi.stubGlobal('localStorage', storage)
    const update = vi.fn()
    const controller = createPaneViewPreferences({
      prefs,
      left: { path: '/left', defaultView: null },
      right: { path: '/right', defaultView: null },
      active: 'left',
      update,
    })

    controller.updateToolbar({ hidden: true })

    expect(loadFolderView('/left')).toBeNull()
    expect(loadFolderView('/right')).toBeNull()
    expect(update).toHaveBeenCalledWith({ hidden: true })
  })
})
