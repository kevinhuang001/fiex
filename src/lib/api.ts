import { translateMessage } from '@/i18n'
import { t } from '@/i18n'
import { invoke, isTauri, convertFileSrc, Channel } from '@tauri-apps/api/core'
import { decodeDynamicIpcResult, serializeIpcJson, parseSearchBatch } from './ipcValidation'
import type { CoreMenu } from '@/components/explorer/context-menu/useContextMenu'
import type { PluginMenu } from '@/components/plugins/PluginMenuNode'
import type {
  StateGetArgs,
  StateGetResult,
  TauriCommand,
  TauriCommandArgs,
  TauriCommandResult,
} from './tauriCommandMap'
import type { Batch } from './types'
export const native = isTauri()
const e2eFixtures = import.meta.env.DEV && import.meta.env.VITE_FIEX_E2E === '1'
export const asset = (path: string) => (native && !path.includes('://') ? convertFileSrc(path) : '')
type CommandCallArgs<C extends TauriCommand> = TauriCommandArgs[C] extends {
  readonly __noArgs?: never
}
  ? [args?: TauriCommandArgs[C]]
  : [args: TauriCommandArgs[C]]
type CoreMenuResolveArgs = Omit<TauriCommandArgs['plugin_context_menu_resolve'], 'source'> & {
  source: 'core'
}
type PluginMenuResolveArgs = Omit<TauriCommandArgs['plugin_context_menu_resolve'], 'source'> & {
  source: 'plugins'
}
export function call<A extends StateGetArgs>(
  command: 'state_get',
  args: A,
): Promise<StateGetResult<A>>
export function call(
  command: 'plugin_context_menu_resolve',
  args: CoreMenuResolveArgs,
): Promise<CoreMenu[]>
export function call(
  command: 'plugin_context_menu_resolve',
  args: PluginMenuResolveArgs,
): Promise<PluginMenu[]>
export function call<C extends TauriCommand>(
  command: C,
  ...args: CommandCallArgs<C>
): Promise<TauriCommandResult<C>>
export async function call<C extends TauriCommand>(
  command: C,
  ...received: [args?: object]
): Promise<TauriCommandResult<C>> {
  const supplied = (received[0] ?? {}) as Record<string, unknown>
  const jsonField =
    command === 'plugin_invoke'
      ? 'params'
      : command === 'plugin_storage_set' || command === 'state_put'
        ? 'value'
        : null
  const args =
    jsonField && jsonField in supplied
      ? { ...supplied, [jsonField]: serializeIpcJson(supplied[jsonField]) }
      : supplied
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
    // Tauri transports command results as JSON. Mock handlers return live JS
    // objects, so remove undefined optional fields the same way the wire does
    // before applying the runtime validators.
    if (value !== undefined) {
      const serialized = JSON.stringify(value)
      if (serialized !== undefined) value = JSON.parse(serialized) as unknown
    }
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
  if (e2eFixtures) {
    return import('../../e2e/fixtures')
      .then(({ mock }) =>
        mock('search_files', { pane, root, query, hidden, ignores, mode, engine }),
      )
      .then(parseSearchBatch)
      .then(onBatch)
  }
  const onBatchChannel = new Channel<unknown>()
  onBatchChannel.onmessage = (batch) => onBatch(parseSearchBatch(batch))
  return invoke('search_files', {
    request: { root, query, hidden, ignores, pane, mode, engine },
    onBatch: onBatchChannel,
  })
}
