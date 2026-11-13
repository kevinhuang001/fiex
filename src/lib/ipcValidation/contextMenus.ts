import type { CoreMenu, CoreMenuItem, MenuChildItem } from '../coreMenu'
import { SHORTCUT_ACTIONS, type ShortcutAction } from '../shortcuts'
import { validPluginMenuItems, type PluginMenu, type PluginMenuItem } from '../pluginMenu'
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
    items: parseCoreMenuItems(menu.items),
  }
}

/** Decode Core submenu items with the same DTO checks used for root menus. */
export function parseCoreMenuItems(value: unknown): CoreMenuItem[] {
  const items = parseJsonValue(value)
  if (!Array.isArray(items) || items.length > 100)
    throw new TypeError('Invalid Core context menu items')
  return items.map(parseCoreMenuItem)
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

/** Decode the shared lazy-child wire shape before UI code knows its owner. */
export function parseContextMenuChildItems(value: unknown): MenuChildItem[] {
  const items = parseJsonValue(value)
  if (!Array.isArray(items) || items.length > 100)
    throw new TypeError('Invalid context submenu items')

  return items.map((value) => {
    const item = objectResult(value)
    const activity = item.activity
    if (
      !hasOnlyKeys(item, [
        'id',
        'kind',
        'title',
        'parentId',
        'icon',
        'shortcut',
        'disabled',
        'danger',
        'placement',
        'activity',
      ]) ||
      typeof item.id !== 'string' ||
      typeof item.title !== 'string' ||
      (item.kind !== 'action' && item.kind !== 'submenu' && item.kind !== 'separator') ||
      (item.parentId !== undefined && typeof item.parentId !== 'string') ||
      (item.icon !== undefined && typeof item.icon !== 'string') ||
      (item.shortcut !== undefined && typeof item.shortcut !== 'string') ||
      (item.disabled !== undefined && typeof item.disabled !== 'boolean') ||
      (item.danger !== undefined && typeof item.danger !== 'boolean') ||
      (item.placement !== undefined && item.placement !== 'tail')
    )
      throw new TypeError('Invalid context submenu item')

    return {
      id: item.id,
      kind: item.kind,
      title: item.title,
      ...(typeof item.parentId === 'string' ? { parentId: item.parentId } : {}),
      ...(typeof item.icon === 'string' ? { icon: item.icon } : {}),
      ...(typeof item.shortcut === 'string' ? { shortcut: item.shortcut } : {}),
      ...(typeof item.disabled === 'boolean' ? { disabled: item.disabled } : {}),
      ...(typeof item.danger === 'boolean' ? { danger: item.danger } : {}),
      ...(item.placement === 'tail' ? { placement: item.placement } : {}),
      ...(activity === undefined ? {} : { activity: parseCoreMenuActivity(activity) }),
    }
  })
}

function parsePluginMenu(value: unknown): PluginMenu {
  const menu = objectResult(value)
  if (
    !hasOnlyKeys(menu, ['kind', 'providerId', 'pluginId', 'menuSessionId', 'items']) ||
    menu.kind !== 'plugin' ||
    typeof menu.providerId !== 'string' ||
    typeof menu.pluginId !== 'string' ||
    typeof menu.menuSessionId !== 'string' ||
    !Array.isArray(menu.items)
  )
    throw new TypeError('Invalid context menu')

  let items: PluginMenuItem[]
  try {
    items = parsePluginMenuItems(menu.items)
  } catch {
    throw new TypeError('Invalid context menu')
  }
  if (!validPluginMenuItems(items)) throw new TypeError('Invalid context menu')

  return {
    kind: 'plugin',
    providerId: menu.providerId,
    pluginId: menu.pluginId,
    menuSessionId: menu.menuSessionId,
    items,
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

/** Decode plugin submenu rows; their parent may be in an earlier lazy page. */
export function parsePluginMenuItems(value: unknown): PluginMenuItem[] {
  const items = parseJsonValue(value)
  if (!Array.isArray(items) || items.length > 100)
    throw new TypeError('Invalid plugin context menu items')

  const decoded = items.map(parsePluginMenuItem)
  const ids = new Set<string>()
  for (const item of decoded) {
    if (!/^[\w.-]{1,80}$/.test(item.id) || ids.has(item.id) || item.title.length > 128)
      throw new TypeError('Invalid plugin context menu item')
    ids.add(item.id)
  }
  return decoded
}

function parsePluginMenuItem(value: unknown): PluginMenuItem {
  const item = objectResult(value)
  if (
    !hasOnlyKeys(item, PLUGIN_MENU_ITEM_KEYS) ||
    typeof item.id !== 'string' ||
    typeof item.title !== 'string' ||
    item.title.length > 128 ||
    !/^[\w.-]{1,80}$/.test(item.id) ||
    (item.kind !== 'action' && item.kind !== 'submenu' && item.kind !== 'separator') ||
    (item.parentId !== undefined && typeof item.parentId !== 'string') ||
    (item.icon !== undefined && typeof item.icon !== 'string') ||
    (item.disabled !== undefined && typeof item.disabled !== 'boolean') ||
    (item.danger !== undefined && typeof item.danger !== 'boolean') ||
    (item.shortcut !== undefined && typeof item.shortcut !== 'string')
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
