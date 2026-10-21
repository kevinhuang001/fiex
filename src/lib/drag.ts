import { Channel } from '@tauri-apps/api/core'
import { startDrag } from '@crabnebula/tauri-plugin-drag'
import { call } from './api'

export const DRAG_ICON =
  'data:image/png;base64,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'

let platform: Promise<string> | undefined
export async function startFileDrag(
  paths: string[],
  onResult: (result: 'Dropped' | 'Cancelled') => void,
  copyRequested = false,
) {
  const exported = await call<string[]>('register_drag', { paths, copyRequested })
  const completed = (result: 'Dropped' | 'Cancelled') => {
    void call('ci_log', { message: `drag result ${result}` }).catch(() => {})
    void call('finish_drag').catch(() => {})
    onResult(result)
  }
  try {
    platform ??= call<string>('platform')
    const kind = await platform
    void call('ci_log', {
      message: `drag start platform=${kind} exported=${JSON.stringify(exported)}`,
    }).catch(() => {})
    if (kind === 'linux') {
      const onEvent = new Channel<string>()
      onEvent.onmessage = (result) => completed(result === 'Dropped' ? 'Dropped' : 'Cancelled')
      await call('drag_files', { paths: exported, onEvent })
    } else {
      await startDrag({ item: exported, icon: DRAG_ICON, mode: 'copy' }, (event) =>
        completed(event.result),
      )
    }
  } catch (error) {
    platform = undefined
    void call('ci_log', { message: `drag error ${String(error)}` }).catch(() => {})
    await call('finish_drag').catch(() => {})
    throw error
  }
}
