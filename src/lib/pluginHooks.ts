import { call } from './api'
import type { PluginInfo } from './plugins'
import type { Entry } from './types'

/** A plugin hook that runs before Core opens a file. Higher priority runs
 * first; a hook returns `handled: true` to consume the open, or anything else
 * to let the next hook (and finally Core's default open) run. */
export interface BeforeOpenHook {
  id: string
  pluginId: string
  pluginName: string
  method: string
  priority: number
  extensions: string[]
}

const dirname = (path: string) => {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index > 0 ? path.slice(0, index) : path
}

export function beforeOpenHooks(plugins: PluginInfo[], entry: Entry): BeforeOpenHook[] {
  const extension = entry.extension.toLowerCase()
  const hooks: BeforeOpenHook[] = []
  for (const plugin of plugins) {
    if (!plugin.enabled || !plugin.approved) continue
    if (!(plugin.grantedPermissions || []).includes('hooks')) continue
    for (const hook of plugin.contributions?.hooks || []) {
      if (hook.event !== 'beforeOpen' || !hook.method) continue
      const extensions = (hook.extensions || []).map((item) =>
        item.toLowerCase().replace(/^\./, ''),
      )
      if (extensions.length && !extensions.includes(extension)) continue
      hooks.push({
        id: `${plugin.id}:${hook.id}`,
        pluginId: plugin.id,
        pluginName: plugin.name,
        method: hook.method,
        priority: hook.priority ?? 100,
        extensions,
      })
    }
  }
  return hooks.sort((a, b) => b.priority - a.priority || a.id.localeCompare(b.id))
}

export interface BeforeOpenOutcome {
  /** A hook consumed the open; Core must not run the default action. */
  handled: boolean
  /** The hook asks Core to open the preview sheet instead (e.g. a password). */
  preview?: boolean
}

/** Hooks run in priority order; each may pass, consume the open, or ask for the
 * preview sheet. Hook failures are recorded and the chain continues: a broken
 * hook must never make a file unopenable. */
export async function runBeforeOpenHooks(
  plugins: PluginInfo[],
  entry: Entry,
): Promise<BeforeOpenOutcome> {
  const directory = dirname(entry.path)
  for (const hook of beforeOpenHooks(plugins, entry)) {
    try {
      const result = await call<{ handled?: boolean; preview?: boolean }>('plugin_invoke', {
        pluginId: hook.pluginId,
        method: hook.method,
        params: {
          path: entry.path,
          name: entry.name,
          extension: entry.extension,
          directory,
        },
        directory,
      })
      if (result?.handled) return { handled: true, preview: Boolean(result.preview) }
    } catch (error) {
      void call('plugin_error_report', {
        pluginId: hook.pluginId,
        source: 'plugin',
        message: `${hook.method}: ${String(error).slice(0, 400)}`,
      }).catch(() => {})
    }
  }
  return { handled: false }
}
