import { translateMessage } from '@/i18n'
import { t } from '@/i18n'
import { invoke, isTauri, convertFileSrc, Channel } from '@tauri-apps/api/core'
import type { Batch } from './types'
export const native = isTauri()
const e2eFixtures = import.meta.env.DEV && import.meta.env.VITE_FIEX_E2E === '1'
export const asset = (path: string) => (native && !path.includes('://') ? convertFileSrc(path) : '')
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (native)
    return invoke<T>(command, args).catch((error: unknown) =>
      Promise.reject(translateMessage(String(error))),
    )
  if (e2eFixtures) {
    const { mock } = await import('../../e2e/fixtures')
    return mock(command, args) as Promise<T>
  }
  throw new Error(t('请在 Fiex 桌面应用中浏览本地文件。'))
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
  const onBatchChannel = new Channel<Batch>()
  onBatchChannel.onmessage = onBatch
  return invoke('search_files', {
    request: { root, query, hidden, ignores, pane, mode, engine },
    onBatch: onBatchChannel,
  })
}
