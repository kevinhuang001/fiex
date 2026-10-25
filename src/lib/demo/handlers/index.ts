import { handleFiles } from './files'
import { handleGit } from './git'
import { handlePlugins } from './plugins'
import { handleSystem } from './system'
import { notHandled } from './shared'

export async function mock(command: string, a: Record<string, unknown>): Promise<unknown> {
  const files = await handleFiles(command, a)
  if (files !== notHandled) return files
  const plugins = await handlePlugins(command, a)
  if (plugins !== notHandled) return plugins
  const git = handleGit(command, a)
  if (git !== notHandled) return git
  const system = handleSystem(command, a)
  if (system !== notHandled) return system
  return null
}
