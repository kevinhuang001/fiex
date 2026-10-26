import { t } from '@/i18n'
import type { Explorer } from '@/hooks/useExplorer'
import { call } from '@/lib/api'
import { basename } from '@/lib/files'
import type { Entry, Location, Preferences } from '@/lib/types'
import type { Notify } from './types'

/** Workspace and bookmark actions: dropping entries into the workspace,
 *  favouriting folders and refreshing every visible pane. */
export function useWorkspaceActions({
  x,
  prefs,
  update,
  workspace,
  notify,
  first,
  left,
  right,
  split,
}: {
  x: Explorer
  prefs: Preferences
  update: (patch: Partial<Preferences>) => void
  workspace: { add: (entries: Entry[]) => void }
  notify: Notify
  first: Entry | undefined
  left: Explorer
  right: Explorer
  split: boolean
}) {
  const addWorkspace = async (paths: string[]) => {
    const entries = await Promise.all(
      paths.map(
        async (path) =>
          x.entries.find((entry) => entry.path === path) ||
          (await call<Entry>('file_info', { path }).catch(() => null)),
      ),
    )
    workspace.add(entries.filter((entry): entry is Entry => Boolean(entry)))
  }
  const refreshAll = () => {
    left.reload()
    if (split) right.reload()
    void left.refreshLocations().catch((error) => x.setError(String(error)))
  }
  const bookmarkPaths = async (paths: string[]) => {
    const existing = new Set(prefs.favorites.map((item) => item.path))
    const additions: Location[] = []
    let files = 0
    for (const path of paths) {
      if (existing.has(path)) continue
      try {
        const info = await call<Entry>('file_info', { path })
        if (!info.isDir) {
          files += 1
          continue
        }
        existing.add(path)
        additions.push({
          name: info.name,
          path,
          kind: 'favorite',
          available: 0,
          total: 0,
          removable: false,
          filesystem: '',
          device: '',
          mounted: true,
          usbfs: false,
        })
      } catch {
        /* The item may have disappeared while dragging. */
      }
    }
    if (additions.length) {
      update({ favorites: [...prefs.favorites, ...additions] })
      notify(t('已收藏 {0} 个文件夹', additions.length))
    } else if (files) {
      notify(t('只能把文件夹加入收藏'))
    } else {
      notify(t('这些文件夹已在收藏中'))
    }
  }
  const favorite = () => {
    const path = first?.isDir ? first.path : x.path
    if (prefs.favorites.some((l) => l.path === path)) {
      update({ favorites: prefs.favorites.filter((l) => l.path !== path) })
      notify(t('已移除收藏'))
    } else {
      update({
        favorites: [
          ...prefs.favorites,
          {
            name: basename(path),
            path,
            kind: 'favorite',
            available: 0,
            total: 0,
            removable: false,
            filesystem: '',
            device: '',
            mounted: true,
            usbfs: false,
          },
        ],
      })
      notify(t('已添加到收藏'))
    }
  }
  return {
    addWorkspace,
    refreshAll,
    bookmarkPaths,
    favorite,
  }
}
