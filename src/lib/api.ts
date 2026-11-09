import { translateMessage } from '@/i18n'
import { t } from '@/i18n'
import { invoke, isTauri, convertFileSrc, Channel } from '@tauri-apps/api/core'
import { decodeDynamicIpcResult, parseSearchBatch } from './ipcValidation'
import type { CoreMenu } from '@/components/explorer/context-menu/useContextMenu'
import type { PluginMenu } from '@/components/plugins/PluginMenuNode'
import type {
  StateGetArgs,
  StateGetResult,
  TauriCommand,
  TauriCommandResult,
} from './tauriCommandMap'
import type { Batch } from './types'
export const native = isTauri()
const e2eFixtures = import.meta.env.DEV && import.meta.env.VITE_FIEX_E2E === '1'
export const asset = (path: string) => (native && !path.includes('://') ? convertFileSrc(path) : '')
export function call<A extends StateGetArgs>(
  command: 'state_get',
  args: A,
): Promise<StateGetResult<A>>
export function call(
  command: 'plugin_context_menu_resolve',
  args: Record<string, unknown> & { source: 'core' },
): Promise<CoreMenu[]>
export function call(
  command: 'plugin_context_menu_resolve',
  args: Record<string, unknown> & { source: 'plugins' },
): Promise<PluginMenu[]>
export function call<C extends TauriCommand>(
  command: C,
  args?: Record<string, unknown>,
): Promise<TauriCommandResult<C>>
export async function call<C extends TauriCommand>(
  command: C,
  args: Record<string, unknown> = {},
): Promise<TauriCommandResult<C>> {
  let value: unknown
  if (native) {
    try {
      value = await invoke<unknown>(command, args)
    } catch (error: unknown) {
      throw translateMessage(String(error))
    }
  } else if (e2eFixtures) {
    const { mock } = await import('../../e2e/fixtures')
    value = await mock(command, args)
  } else {
    throw new Error(t('请在 Fiex 桌面应用中浏览本地文件。'))
  }
  return decodeDynamicIpcResult(command, args, value) as TauriCommandResult<C>
}
export function search(
  pane: string,
  root: string,
  query: string,
  hidden: boolean,
  ignores: string[],
  mode: 'name' | 'content',
  engine: string,
  onBatch: (b: Batch) => void,
): Promise<void> {
  if (e2eFixtures)
    return call('search_files', { pane, root, query, hidden, ignores, mode, engine }).then(onBatch)
  const onBatchChannel = new Channel<unknown>()
  onBatchChannel.onmessage = (batch) => onBatch(parseSearchBatch(batch))
  return invoke('search_files', {
    request: { root, query, hidden, ignores, pane, mode, engine },
    onBatch: onBatchChannel,
  })
}
