import { invoke, isTauri, convertFileSrc, Channel } from '@tauri-apps/api/core'
import type { Batch } from './types'
export const native = isTauri()
export const demo = !native && new URLSearchParams(location.search).has('demo')
export const asset = (path: string) => (native ? convertFileSrc(path) : '')
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (native) return invoke<T>(command, args)
  if (demo) {
    const { mock } = await import('./demo')
    return mock(command, args) as Promise<T>
  }
  throw new Error('请在 Fiex 桌面应用中浏览本地文件。')
}
export function search(
  root: string,
  query: string,
  hidden: boolean,
  respectIgnore: boolean,
  onBatch: (b: Batch) => void,
): Promise<void> {
  if (!native) return call<Batch>('search_files', { root, query, hidden }).then(onBatch)
  const onBatchChannel = new Channel<Batch>()
  onBatchChannel.onmessage = onBatch
  return invoke('search_files', { root, query, hidden, respectIgnore, onBatch: onBatchChannel })
}
