import { t } from '@/i18n'
import { call } from '@/lib/api'
import type { PluginInfo } from '@/lib/plugins'
import {
  beginActivity,
  finishActivity,
  quietProgressChannel,
  updateActivity,
} from '@/hooks/useActivity'
import { OPEN_PLUGIN_PANEL, PLUGIN_RESULT_EVENT } from '@/components/plugins/PluginPanelHost'
import type { PluginMenu, PluginMenuItem } from './PluginMenuNode'

export async function invokePluginContextMenuItem({
  menu,
  item,
  paths,
  path,
  plugins,
  anchor,
  onClose,
  onError,
}: {
  menu: PluginMenu
  item: PluginMenuItem
  paths: string[]
  path: string
  plugins: PluginInfo[]
  anchor?: { x: number; y: number }
  onClose: () => void
  onError: (error: string) => void
}) {
  onClose()
  const pluginId = menu.pluginId
  const plugin = plugins.find((entry) => entry.id === pluginId)
  let activityId = ''
  try {
    const response = await call('plugin_context_menu_execute', {
      menuSessionId: menu.menuSessionId,
      itemId: item.id,
      onProgress: quietProgressChannel(),
    })
    if (
      response &&
      typeof response === 'object' &&
      'kind' in response &&
      response.kind === 'panel' &&
      'assetPath' in response &&
      typeof response.assetPath === 'string'
    ) {
      const entry = response as {
        assetPath: string
        panelId?: string
        title?: string
        size?: { width: number; height: number }
      }
      window.dispatchEvent(
        new CustomEvent(OPEN_PLUGIN_PANEL, {
          detail: {
            pluginId,
            panelId: entry.panelId || item.id,
            title: entry.title || item.title,
            icon: plugin?.icon,
            selection: paths,
            directory: path,
            anchor,
            source: 'contextMenu',
            size: entry.size,
            assetPath: entry.assetPath,
            timeoutMs: plugin?.uiTimeoutMs,
            allowCreate: (plugin?.declaredPermissions || []).some(
              (permission) => permission === 'files.create' || permission === 'folders.create',
            ),
          },
        }),
      )
      return
    }
    activityId = beginActivity(
      (plugin?.name || t('插件')) + ' · ' + item.title,
      paths.length + t(' 个项目'),
      undefined,
      'plugin',
      plugin?.id,
    )
    updateActivity(activityId, { indeterminate: true })
    finishActivity(activityId, undefined, t('已提交'))
    const silent =
      response && typeof response === 'object' && ('taskId' in response || 'handled' in response)
    if (!silent && response && typeof response === 'object') {
      window.dispatchEvent(
        new CustomEvent(PLUGIN_RESULT_EVENT, { detail: { title: item.title, result: response } }),
      )
    }
  } catch (error) {
    if (activityId) finishActivity(activityId, String(error))
    onError(String(error))
  }
}
