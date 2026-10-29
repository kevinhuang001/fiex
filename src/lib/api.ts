import { translateMessage } from '@/i18n'
import { t } from '@/i18n'
import { invoke, isTauri, convertFileSrc, Channel } from '@tauri-apps/api/core'
import type { Batch } from './types'
export const native = isTauri()
export const demo =
  !native &&
  (import.meta.env.DEV ||
    ['demo', 'pluginFixture', 'officialPlugins'].some((key) =>
      new URLSearchParams(location.search).has(key),
    ))
export const asset = (path: string) => (native && !path.includes('://') ? convertFileSrc(path) : '')
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (native)
    return invoke<T>(command, args).catch((error: unknown) =>
      Promise.reject(translateMessage(String(error))),
    )
  if (demo) {
    const { mock } = await import('./demo')
    return mock(command, args) as Promise<T>
  }
  throw new Error(t('请在 Fiex 桌面应用中浏览本地文件。'))
}
export function search(
  pane: string,
  root: string,
  query: string,
  mode: 'name' | 'content',
  engine: string,
  remoteContentApproved: boolean,
  hidden: boolean,
  ignores: string[],
  onBatch: (b: Batch) => void,
): Promise<void> {
  if (!native)
    return call<Batch>('search_files', {
      root,
      query,
      mode,
      engine,
      remoteContentApproved,
      hidden,
      ignores,
      pane,
    }).then(onBatch)
  const onBatchChannel = new Channel<Batch>()
  onBatchChannel.onmessage = onBatch
  return invoke('search_files', {
    request: { root, query, mode, engine, remoteContentApproved, hidden, ignores, pane },
    onBatch: onBatchChannel,
  })
}
