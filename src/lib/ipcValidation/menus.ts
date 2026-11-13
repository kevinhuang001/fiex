import type { ConnectionMenu } from '../connectionMenu'
import type { CoreMenu } from '../coreMenu'
import type { PluginMenu } from '../pluginMenu'
import { isJsonObject, objectResult, parseJsonValue } from './json'
import { validPluginMenuItems } from '../pluginMenu'
import type { ConnectionMenuItem } from '../connectionMenu'

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
  return json.map(parseConnectionMenu)
}

function parseConnectionMenu(value: unknown): ConnectionMenu {
  const menu = objectResult(value)
  const pluginId = menu.pluginId
  const pluginName = menu.pluginName
  if (
    (menu.kind !== 'builtin' && menu.kind !== 'plugin') ||
    typeof menu.providerId !== 'string' ||
    typeof menu.menuSessionId !== 'string' ||
    (menu.kind === 'plugin' && typeof pluginId !== 'string') ||
    (pluginId !== undefined && typeof pluginId !== 'string') ||
    (pluginName !== undefined && typeof pluginName !== 'string') ||
    !Array.isArray(menu.items)
  )
    throw new TypeError('Invalid connection menu')

  return {
    kind: menu.kind,
    providerId: menu.providerId,
    menuSessionId: menu.menuSessionId,
    ...(typeof pluginId === 'string' ? { pluginId } : {}),
    ...(typeof pluginName === 'string' ? { pluginName } : {}),
    items: menu.items.map(parseConnectionMenuItem),
  }
}

function parseConnectionMenuItem(value: unknown): ConnectionMenuItem {
  const item = objectResult(value)
  const icon = item.icon
  const shortcut = item.shortcut
  const danger = item.danger
  const disabled = item.disabled
  const placement = item.placement
  const activity = item.activity
  if (
    typeof item.id !== 'string' ||
    typeof item.title !== 'string' ||
    (item.kind !== 'action' && item.kind !== 'separator') ||
    (icon !== undefined && typeof icon !== 'string') ||
    (shortcut !== undefined && typeof shortcut !== 'string') ||
    (danger !== undefined && typeof danger !== 'boolean') ||
    (disabled !== undefined && typeof disabled !== 'boolean') ||
    (placement !== undefined && placement !== 'tail')
  )
    throw new TypeError('Invalid connection menu item')

  return {
    id: item.id,
    title: item.title,
    kind: item.kind,
    ...(typeof icon === 'string' ? { icon } : {}),
    ...(typeof shortcut === 'string' ? { shortcut } : {}),
    ...(typeof danger === 'boolean' ? { danger } : {}),
    ...(typeof disabled === 'boolean' ? { disabled } : {}),
    ...(placement === 'tail' ? { placement } : {}),
    ...(activity === undefined ? {} : { activity: parseConnectionMenuActivity(activity) }),
  }
}

function parseConnectionMenuActivity(value: unknown): NonNullable<ConnectionMenuItem['activity']> {
  const activity = objectResult(value)
  if (
    typeof activity.label !== 'string' ||
    (activity.kind !== 'sync' && activity.kind !== 'transfer') ||
    typeof activity.quiet !== 'boolean'
  )
    throw new TypeError('Invalid connection menu activity')
  return { label: activity.label, kind: activity.kind, quiet: activity.quiet }
}
