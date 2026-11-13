const STRING_ARRAY_COMMANDS = new Set([
  'plugin_files_dialog',
  'plugin_network_domains',
  'plugin_release_assets',
  'register_drag',
  'system_fonts',
])

export function decodeSimpleCollectionIpcResult(
  command: string,
  value: unknown,
): { value: unknown } | null {
  if (STRING_ARRAY_COMMANDS.has(command)) {
    if (!Array.isArray(value) || !value.every((item) => typeof item === 'string'))
      throw new TypeError(`Invalid ${command} IPC result`)
    return { value: [...value] }
  }

  if (command === 'plugin_status') {
    if (!Array.isArray(value)) throw new TypeError('Invalid plugin_status IPC result')
    const statuses: Array<[string, boolean]> = []
    for (const item of value) {
      if (!Array.isArray(item) || item.length !== 2) {
        throw new TypeError('Invalid plugin_status IPC result')
      }
      const [pluginId, enabled] = item
      if (typeof pluginId !== 'string' || typeof enabled !== 'boolean') {
        throw new TypeError('Invalid plugin_status IPC result')
      }
      statuses.push([pluginId, enabled])
    }
    return { value: statuses }
  }

  return null
}
