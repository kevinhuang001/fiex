import { t } from '@/i18n'
import { useSyncExternalStore } from 'react'
import { call, native } from '@/lib/api'
import type { JsonValue } from '../../sdk/ui/src/index'
import type { MarketplacePlugin, PluginExtensions, PluginInfo } from '@/shared/ipc/contracts/plugin'
export type {
  MarketplacePlugin,
  PluginAuthor,
  PluginContribution,
  PluginExtensions,
  PluginField,
  PluginInfo,
} from '@/shared/ipc/contracts/plugin'
export const PLUGIN_CATEGORIES = [
  {
    id: 'info',
    get label() {
      return t('简介')
    },
  },
  {
    id: 'preview',
    get label() {
      return t('预览')
    },
  },
  {
    id: 'contextMenus',
    get label() {
      return t('右键菜单')
    },
  },
  {
    id: 'toolbar',
    get label() {
      return t('工具栏')
    },
  },
  {
    id: 'search',
    get label() {
      return t('搜索')
    },
  },
  {
    id: 'connections',
    get label() {
      return t('连接')
    },
  },
  {
    id: 'hooks',
    get label() {
      return t('打开方式')
    },
  },
  {
    id: 'themes',
    get label() {
      return t('主题')
    },
  },
  {
    id: 'commands',
    get label() {
      return t('命令')
    },
  },
] as const
export function pluginCategories(
  extensions?: PluginExtensions | JsonValue | null,
  commands: string[] = [],
): string[] {
  const extensionObject =
    extensions && typeof extensions === 'object' && !Array.isArray(extensions)
      ? (extensions as unknown as Record<string, unknown>)
      : undefined
  const present = PLUGIN_CATEGORIES.filter(({ id }) => {
    if (id === 'commands') return false
    // A plugin that only decorates connection menus still belongs to the
    // connection family.
    if (id === 'connections' && extensionObject?.connectionContextMenus) return true
    const value = extensionObject?.[id]
    return Boolean(value) && (Array.isArray(value) ? value.length > 0 : true)
  }).map(({ id }) => id)
  if (!present.length && commands.length) present.push('commands')
  return present
}
export function isNewerVersion(candidate: string, installed: string): boolean {
  const left = candidate.split('-')[0].split('.').map(Number)
  const right = installed.split('-')[0].split('.').map(Number)
  for (let index = 0; index < Math.max(left.length, right.length); index++) {
    const a = left[index] || 0
    const b = right[index] || 0
    if (a !== b) return a > b
  }
  return false
}
async function pluginCatalog(): Promise<PluginInfo[]> {
  if (!native && !(import.meta.env.DEV && import.meta.env.VITE_FIEX_E2E === '1')) return []
  return call('plugins_list')
}
let catalogSnapshot: PluginInfo[] = []
const catalogListeners = new Set<() => void>()
let stopCatalog: (() => void) | undefined
let catalogRequest: Promise<void> | undefined
let refreshAgain = false

function reloadCatalog() {
  if (catalogRequest) {
    refreshAgain = true
    return catalogRequest
  }
  catalogRequest = pluginCatalog()
    .then((plugins) => {
      // Stable snapshots avoid rendering every consumer for unchanged scans.
      if (JSON.stringify(plugins) === JSON.stringify(catalogSnapshot)) return
      catalogSnapshot = plugins
      catalogListeners.forEach((notify) => notify())
    })
    .catch(() => {})
    .finally(() => {
      catalogRequest = undefined
      if (refreshAgain) {
        refreshAgain = false
        reloadCatalog()
      }
    })
  return catalogRequest
}
function subscribeCatalog(notify: () => void) {
  catalogListeners.add(notify)
  if (!stopCatalog) {
    const refresh = () => reloadCatalog()
    const poll = window.setInterval(() => {
      if (document.visibilityState !== 'hidden') reloadCatalog()
    }, 3000)
    window.addEventListener('fiex:plugins-changed', refresh)
    window.addEventListener('focus', refresh)
    stopCatalog = () => {
      clearInterval(poll)
      window.removeEventListener('fiex:plugins-changed', refresh)
      window.removeEventListener('focus', refresh)
    }
    reloadCatalog()
  }
  return () => {
    catalogListeners.delete(notify)
    if (!catalogListeners.size) {
      stopCatalog?.()
      stopCatalog = undefined
    }
  }
}
/** One shared catalog and one discovery subscription for the whole UI. */
export function usePluginCatalog(): PluginInfo[] {
  return useSyncExternalStore(subscribeCatalog, () => catalogSnapshot)
}
export async function syncPluginCatalog() {
  if (catalogRequest) await catalogRequest
  await reloadCatalog()
}
export function refreshPluginCatalog() {
  window.dispatchEvent(new Event('fiex:plugins-changed'))
}

export async function pluginMarketplace(): Promise<MarketplacePlugin[]> {
  return call('plugin_marketplace', {
    repository: 'kevinhuang001/fiex-official-plugins',
  })
}
