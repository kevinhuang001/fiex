import { progressChannel, quietProgressChannel } from '@/hooks/useActivity'
import { call } from '@/lib/api'
import { reportFilesystemPermissionError } from '@/lib/errorCenter'
import { parseCoreMenuItems, parsePluginMenuResult } from '@/lib/ipcValidation/contextMenus'
import { requestDirectoryRefresh } from '@/lib/listing'
import type {
  ContextMenuExecutionServices,
  ContextMenuSessionServices,
} from '@/features/context-menu/types'

/** App and Core effects stay at the component boundary; the feature receives typed services. */
export const contextMenuSessionServices: ContextMenuSessionServices = {
  newSessionId: () => crypto.randomUUID(),
  resolveCoreMenus: async ({ directory, paths, sessionId }) =>
    parsePluginMenuResult(
      await call('plugin_context_menu_resolve', {
        directory,
        paths,
        source: 'core',
        sessionId,
      }),
      'core',
    ),
  resolvePluginMenus: async ({ directory, paths, sessionId }) =>
    parsePluginMenuResult(
      await call('plugin_context_menu_resolve', {
        directory,
        paths,
        source: 'plugins',
        sessionId,
      }),
      'plugins',
    ),
  resolveCoreChildren: async (menuSessionId, parentId) => {
    const response = await call('plugin_context_menu_children', { menuSessionId, parentId })
    return parseCoreMenuItems(response.items)
  },
}

export const contextMenuExecutionServices: ContextMenuExecutionServices = {
  progressChannel,
  quietProgressChannel,
  executeMenu: (menuSessionId, itemId, onProgress) =>
    call('plugin_context_menu_execute', { menuSessionId, itemId, onProgress }),
  recentNavigationTarget: (alias) => call('recent_navigation_target', { alias }),
  copyText: (value) => navigator.clipboard.writeText(value),
  refreshDirectories: requestDirectoryRefresh,
  reportFilesystemPermissionError,
}
