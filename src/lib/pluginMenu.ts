export interface PluginMenuItem {
  id: string
  title: string
  kind: 'action' | 'submenu' | 'separator'
  parentId?: string
  icon?: string
}

export interface PluginMenu {
  pluginId: string
  menuSessionId: string
  items: PluginMenuItem[]
}

/** Validate untrusted plugin menu trees before they enter the UI. */
export function validPluginMenuItems(items: unknown): items is PluginMenuItem[] {
  if (!Array.isArray(items) || items.length > 100) return false
  const ids = new Set<string>()
  for (const item of items) {
    if (!item || typeof item !== 'object') return false
    const node = item as Record<string, unknown>
    if (
      typeof node.id !== 'string' ||
      !/^[\w.-]{1,80}$/.test(node.id) ||
      ids.has(node.id) ||
      typeof node.title !== 'string' ||
      node.title.length > 128 ||
      !['action', 'submenu', 'separator'].includes(String(node.kind)) ||
      ('parentId' in node && typeof node.parentId !== 'string') ||
      ('icon' in node && typeof node.icon !== 'string')
    )
      return false
    ids.add(node.id)
  }
  const byId = new Map(items.map((item) => [item.id, item as PluginMenuItem]))
  for (const item of items as PluginMenuItem[]) {
    let depth = 1
    let current = item
    const seen = new Set<string>([item.id])
    while (current.parentId) {
      const parent = byId.get(current.parentId)
      if (!parent || parent.kind !== 'submenu' || seen.has(parent.id) || ++depth > 4) return false
      seen.add(parent.id)
      current = parent
    }
  }
  return true
}
