import { getLocale, t } from '@/i18n'
import { call, native, search } from '@/lib/api'
import {
  cachePresentationPatch,
  cachePresentations,
  cachedPresentations,
} from '@/lib/entryPresentation'
import { publishPresentationIcons } from '@/lib/decorationIcons'
import { openDirectoryStream } from '@/lib/listing'
import type { Entry, EntryDecoration } from '@/lib/types'

/** Provider adapters stay at the feature composition boundary. */
export const directorySnapshotServices = {
  openStream: openDirectoryStream,
  cachePresentationPatch,
  cachePresentations,
  publishPresentationIcons,
  targetNotFoundError: () => t('无法定位已打开的文件'),
}

export const listingSearchServices = {
  cancel: (pane: string) => call('cancel_search', { pane }),
  search,
  truncatedMessage: () => t(' · 已达到 500 项上限，请缩小范围'),
  skippedMessage: (count: number) => t(' · {0} 个位置无法访问', count),
}

export const entryDecorationServices = {
  getLocale,
  load: (directory: string, entries: Entry[], includePlugins: boolean) =>
    call('plugin_decorations', {
      directory,
      entries: entries.map((entry) => ({
        path: entry.path,
        name: entry.name,
        isDir: entry.isDir,
        extension: entry.extension,
        mime: entry.mime,
        allowedOps: entry.allowedOps || [],
        tags: entry.tags,
        sync: entry.sync,
        hasCache: entry.hasCache || false,
        pinned: entry.pinned || false,
        uploaded: entry.uploaded || false,
        linkTarget: entry.linkTarget,
      })),
      includePlugins,
    }),
  cache: cachePresentations,
  publishIcons: (entries: Entry[]) => publishPresentationIcons(entries),
  cached: (entries: Entry[]): Record<string, EntryDecoration> => cachedPresentations(entries),
}

export const listingLocationServices = {
  native,
  loadLocations: () => call('locations'),
  loadLaunchPath: () => call('launch_path'),
  loadLastPath: async () => {
    const lastPath = await call('state_get', {
      namespace: 'explorer',
      key: 'lastPath',
    }).catch(() => null)
    return lastPath || localStorage.getItem('fiex:lastPath')
  },
  loadInitialPath: () => call('initial_path'),
  persistLastPath: async (path: string) => {
    try {
      await call('state_put', {
        namespace: 'explorer',
        key: 'lastPath',
        value: path,
      })
      localStorage.removeItem('fiex:lastPath')
    } catch {
      localStorage.setItem('fiex:lastPath', path)
    }
  },
}
