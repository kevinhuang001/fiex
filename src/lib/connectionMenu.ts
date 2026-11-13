export interface ConnectionMenuItem {
  id: string
  kind: 'action' | 'separator'
  title: string
  icon?: string
  shortcut?: string
  danger?: boolean
  disabled?: boolean
  placement?: 'tail'
  activity?: { label: string; kind: 'sync' | 'transfer'; quiet: boolean }
}

export interface ConnectionMenu {
  kind: 'builtin' | 'plugin'
  providerId: string
  pluginId?: string
  pluginName?: string
  menuSessionId: string
  items: ConnectionMenuItem[]
}

export interface ConnectionMenuAction {
  kind: 'coreAction'
  command: 'editConnection' | 'closeConnection'
  connectionId: string
}

/** Decode the two privileged connection-menu actions; custom results stay generic. */
export function parseConnectionMenuAction(
  value: unknown,
  expectedConnectionId: string,
): ConnectionMenuAction | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  const action = value as Record<string, unknown>
  if (action.kind !== 'coreAction') return null

  if (
    Object.keys(action).some((key) => !['kind', 'command', 'connectionId'].includes(key)) ||
    (action.command !== 'editConnection' && action.command !== 'closeConnection') ||
    typeof action.connectionId !== 'string' ||
    action.connectionId.length === 0 ||
    action.connectionId !== expectedConnectionId
  )
    throw new TypeError('Invalid connection menu action')

  return {
    kind: 'coreAction',
    command: action.command,
    connectionId: action.connectionId,
  }
}
