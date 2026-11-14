import { Channel } from '@tauri-apps/api/core'
import type { OperationProgress } from './types'

export type ProgressChannelFactory = () => Channel<OperationProgress>

export function quietProgressChannel(): Channel<OperationProgress> {
  return new Channel<OperationProgress>()
}
