import {
  folderViewPreferences,
  hasFolderOrderingChange,
  hasFolderViewChange,
  patchFolderView,
  saveActiveFolderView,
  saveFolderView,
} from '@/features/explorer/model/folderView'
import type { Preferences, View } from '@/lib/types'

interface PaneViewSource {
  path: string
  defaultView: View | null | undefined
}

interface PaneViewPreferencesOptions {
  prefs: Preferences
  left: PaneViewSource
  right: PaneViewSource
  active: 'left' | 'right'
  update: (patch: Partial<Preferences>) => void
}

export function createPaneViewPreferences({
  prefs,
  left,
  right,
  active,
  update,
}: PaneViewPreferencesOptions) {
  const leftPreferences = folderViewPreferences(prefs, left.path, left.defaultView)
  const rightPreferences = folderViewPreferences(prefs, right.path, right.defaultView)

  const updatePane =
    (
      pane: PaneViewSource,
      panePreferences: Preferences,
      otherPane: PaneViewSource,
      otherPreferences: Preferences,
    ) =>
    (patch: Partial<Preferences>) => {
      if (hasFolderViewChange(patch)) {
        if (otherPane.path && otherPane.path !== pane.path)
          saveFolderView(otherPane.path, patchFolderView(otherPreferences, {}))
        if (pane.path) saveFolderView(pane.path, patchFolderView(panePreferences, patch))
      }
      update(patch)
    }

  const updateLeft = updatePane(left, leftPreferences, right, rightPreferences)
  const updateRight = updatePane(right, rightPreferences, left, leftPreferences)
  const activePreferences = active === 'right' ? rightPreferences : leftPreferences
  const updateActive = active === 'right' ? updateRight : updateLeft

  const updateToolbar = (patch: Partial<Preferences>) => {
    if (!hasFolderOrderingChange(patch)) {
      updateActive(patch)
      return
    }

    saveActiveFolderView(
      active,
      {
        left: { path: left.path, preferences: leftPreferences },
        right: { path: right.path, preferences: rightPreferences },
      },
      patch,
    )
    update(patch)
  }

  return {
    left: leftPreferences,
    right: rightPreferences,
    active: activePreferences,
    updateLeft,
    updateRight,
    updateToolbar,
  }
}
