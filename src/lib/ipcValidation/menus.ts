import type { ConnectionMenu } from '../connectionMenu'
import type { CoreMenu, CoreMenuItem } from '../coreMenu'
import { SHORTCUT_ACTIONS, type ShortcutAction } from '../shortcuts'
import { validPluginMenuItems, type PluginMenu, type PluginMenuItem } from '../pluginMenu'
import type { ConnectionMenuItem } from '../connectionMenu'
import { objectResult, parseJsonValue } from './json'

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
  if (source === 'core') return json.map(parseCoreMenu)
  return json.map(parsePluginMenu)
}

function parseCoreMenu(value: unknown): CoreMenu {
  const menu = objectResult(value)
  if (
    !hasOnlyKeys(menu, ['kind', 'providerId', 'menuSessionId', 'items']) ||
    menu.kind !== 'builtin' ||
    typeof menu.providerId !== 'string' ||
    typeof menu.menuSessionId !== 'string' ||
    !Array.isArray(menu.items) ||
    menu.items.length > 100
  )
    throw new TypeError('Invalid context menu')

  return {
    kind: 'builtin',
    providerId: menu.providerId,
    menuSessionId: menu.menuSessionId,
    items: menu.items.map(parseCoreMenuItem),
  }
}

function parseCoreMenuItem(value: unknown): CoreMenuItem {
  const item = objectResult(value)
  const optionalStringFields = ['parentId', 'icon'] as const
  const optionalBooleanFields = ['disabled', 'danger'] as const
  const shortcut = item.shortcut
  const placement = item.placement
  const activity = item.activity
  if (
    !hasOnlyKeys(item, [
      'id',
      'kind',
      'title',
      ...optionalStringFields,
      'shortcut',
      ...optionalBooleanFields,
      'placement',
      'activity',
    ]) ||
    typeof item.id !== 'string' ||
    typeof item.title !== 'string' ||
    (item.kind !== 'action' && item.kind !== 'submenu' && item.kind !== 'separator') ||
    optionalStringFields.some((key) => item[key] !== undefined && typeof item[key] !== 'string') ||
    (shortcut !== undefined && !isShortcutAction(shortcut)) ||
    optionalBooleanFields.some(
      (key) => item[key] !== undefined && typeof item[key] !== 'boolean',
    ) ||
    (placement !== undefined && placement !== 'tail')
  )
    throw new TypeError('Invalid Core context menu item')

  return {
    id: item.id,
    kind: item.kind,
    title: item.title,
    ...(typeof item.parentId === 'string' ? { parentId: item.parentId } : {}),
    ...(typeof item.icon === 'string' ? { icon: item.icon } : {}),
    ...(isShortcutAction(shortcut) ? { shortcut } : {}),
    ...(typeof item.disabled === 'boolean' ? { disabled: item.disabled } : {}),
    ...(typeof item.danger === 'boolean' ? { danger: item.danger } : {}),
    ...(placement === 'tail' ? { placement } : {}),
    ...(activity === undefined ? {} : { activity: parseCoreMenuActivity(activity) }),
  }
}

function parseCoreMenuActivity(value: unknown): NonNullable<CoreMenuItem['activity']> {
  const activity = objectResult(value)
  if (
    !hasOnlyKeys(activity, ['label', 'kind', 'quiet']) ||
    typeof activity.label !== 'string' ||
    (activity.kind !== 'sync' && activity.kind !== 'transfer') ||
    typeof activity.quiet !== 'boolean'
  )
    throw new TypeError('Invalid Core context menu activity')
  return { label: activity.label, kind: activity.kind, quiet: activity.quiet }
}

function parsePluginMenu(value: unknown): PluginMenu {
  const menu = objectResult(value)
  if (
    !hasOnlyKeys(menu, ['kind', 'providerId', 'pluginId', 'menuSessionId', 'items']) ||
    menu.kind !== 'plugin' ||
    typeof menu.providerId !== 'string' ||
    typeof menu.pluginId !== 'string' ||
    typeof menu.menuSessionId !== 'string' ||
    !Array.isArray(menu.items) ||
    menu.items.length > 100 ||
    menu.items.some((item) => !hasOnlyKeys(objectResult(item), PLUGIN_MENU_ITEM_KEYS)) ||
    !validPluginMenuItems(menu.items)
  )
    throw new TypeError('Invalid context menu')

  return {
    kind: 'plugin',
    providerId: menu.providerId,
    pluginId: menu.pluginId,
    menuSessionId: menu.menuSessionId,
    items: menu.items.map(parsePluginMenuItem),
  }
}

const PLUGIN_MENU_ITEM_KEYS = [
  'id',
  'title',
  'kind',
  'parentId',
  'icon',
  'disabled',
  'danger',
  'shortcut',
] as const

function parsePluginMenuItem(value: unknown): PluginMenuItem {
  const item = objectResult(value)
  if (
    typeof item.id !== 'string' ||
    typeof item.title !== 'string' ||
    (item.kind !== 'action' && item.kind !== 'submenu' && item.kind !== 'separator')
  )
    throw new TypeError('Invalid plugin context menu item')

  return {
    id: item.id,
    title: item.title,
    kind: item.kind,
    ...(typeof item.parentId === 'string' ? { parentId: item.parentId } : {}),
    ...(typeof item.icon === 'string' ? { icon: item.icon } : {}),
    ...(typeof item.disabled === 'boolean' ? { disabled: item.disabled } : {}),
    ...(typeof item.danger === 'boolean' ? { danger: item.danger } : {}),
    ...(typeof item.shortcut === 'string' ? { shortcut: item.shortcut } : {}),
  }
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function isShortcutAction(value: unknown): value is ShortcutAction {
  return typeof value === 'string' && shortcutActions.has(value)
}

const shortcutActions = new Set<string>(SHORTCUT_ACTIONS)

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
