import { handleFiles } from './files'
import { handlePlugins } from './plugins'
import { handleSystem } from './system'
import { notHandled } from './shared'

export async function mock(command: string, a: Record<string, unknown>): Promise<unknown> {
  const files = await handleFiles(command, a)
  if (files !== notHandled) return files
  const plugins = await handlePlugins(command, a)
  if (plugins !== notHandled) return plugins
  const system = handleSystem(command, a)
  if (system !== notHandled) return system
  return null
}
