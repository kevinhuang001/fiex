import type { ConnectionMenu } from '@/components/sidebar/Connections'
import type { CoreMenu } from '../coreMenu'
import type { PluginMenu } from '@/components/plugins/PluginMenuNode'
import { isJsonObject, parseJsonValue } from './json'
import { validPluginMenuItems } from '../pluginMenu'

export function parsePluginMenuResult(value: unknown, source: 'core'): CoreMenu[]
export function parsePluginMenuResult(value: unknown, source: 'plugins'): PluginMenu[]
export function parsePluginMenuResult(
  value: unknown,
  source: 'core' | 'plugins',
): CoreMenu[] | PluginMenu[]
export function parsePluginMenuResult(
  value: unknown,
  source: 'core' | 'plugins',
): CoreMenu[] | PluginMenu[] {
  const json = parseJsonValue(value)
  if (!Array.isArray(json)) throw new TypeError('Invalid context menu response')
  const menus = json.map((menu) => {
    if (menu === null || Array.isArray(menu) || typeof menu !== 'object')
      throw new TypeError('Invalid context menu')
    if (
      typeof menu.menuSessionId !== 'string' ||
      (source === 'core'
        ? menu.kind !== 'builtin' ||
          typeof menu.providerId !== 'string' ||
          !validCoreItems(menu.items)
        : typeof menu.pluginId !== 'string' || !validPluginMenuItems(menu.items))
    )
      throw new TypeError('Invalid context menu')
    return menu
  })
  return menus as unknown as CoreMenu[] | PluginMenu[]
}

function validCoreItems(items: unknown): boolean {
  if (!Array.isArray(items) || items.length > 100) return false
  return items.every(
    (item) =>
      isJsonObject(item) &&
      typeof item.id === 'string' &&
      typeof item.title === 'string' &&
      ['action', 'submenu', 'separator'].includes(String(item.kind)) &&
      (item.parentId === undefined || typeof item.parentId === 'string'),
  )
}

export function parseConnectionMenus(value: unknown): ConnectionMenu[] {
  const json = parseJsonValue(value)
  if (!Array.isArray(json)) throw new TypeError('Invalid connection menu list')
  for (const menu of json) {
    if (
      menu === null ||
      Array.isArray(menu) ||
      typeof menu !== 'object' ||
      !['builtin', 'plugin'].includes(String(menu.kind)) ||
      typeof menu.providerId !== 'string' ||
      typeof menu.menuSessionId !== 'string' ||
      (menu.kind === 'plugin' && typeof menu.pluginId !== 'string') ||
      (menu.pluginName !== undefined && typeof menu.pluginName !== 'string') ||
      !Array.isArray(menu.items)
    )
      throw new TypeError('Invalid connection menu')
    for (const item of menu.items) {
      if (
        item === null ||
        Array.isArray(item) ||
        typeof item !== 'object' ||
        typeof item.id !== 'string' ||
        typeof item.title !== 'string' ||
        !['action', 'separator'].includes(String(item.kind)) ||
        (item.icon !== undefined && typeof item.icon !== 'string') ||
        (item.disabled !== undefined && typeof item.disabled !== 'boolean') ||
        (item.danger !== undefined && typeof item.danger !== 'boolean')
      )
        throw new TypeError('Invalid connection menu item')
    }
  }
  return json as unknown as ConnectionMenu[]
}
