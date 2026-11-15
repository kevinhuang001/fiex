import { Channel } from '@tauri-apps/api/core'
import { call, native } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import type { InfoSessionServices } from '@/features/info/types'

export const infoSessionServices: InfoSessionServices = {
  open: (path, onUpdate) => {
    if (!native) return call('file_info_open', { path })
    const channel = new Channel<unknown>()
    channel.onmessage = onUpdate
    return call('file_info_open', { path, onUpdate: channel })
  },
  compute: ({ sessionId, providerId, subsectionId, fieldId, taskId }) =>
    call('file_info_compute', {
      sessionId,
      providerId,
      fields: [{ subsectionId, fieldId }],
      taskId,
    }),
  cancel: (sessionId, taskId) => call('file_info_cancel', { sessionId, taskId }),
  close: (sessionId) => call('file_info_session_close', { sessionId }),
  reportError,
  createTaskId: () => crypto.randomUUID(),
}
