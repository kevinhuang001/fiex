export type SessionStatus = 'connecting' | 'open' | 'closed' | 'error'
export type TerminalEvent =
  | { type: 'ready' }
  | { type: 'data'; data: number[] }
  | { type: 'exit'; code?: number | null }
  | { type: 'error'; message?: string }

export function parseTerminalEvent(value: unknown): TerminalEvent {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new TypeError('Invalid terminal event')
  const event = value as Record<string, unknown>
  if (event.type === 'ready') return { type: 'ready' }
  if (
    event.type === 'data' &&
    Array.isArray(event.data) &&
    event.data.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
  )
    return { type: 'data', data: event.data as number[] }
  if (
    event.type === 'exit' &&
    (event.code === undefined || event.code === null || typeof event.code === 'number')
  )
    return { type: 'exit', code: event.code as number | null | undefined }
  if (event.type === 'error' && (event.message === undefined || typeof event.message === 'string'))
    return { type: 'error', message: event.message as string | undefined }
  throw new TypeError('Invalid terminal event')
}

/** Startup acknowledgements do not imply a live shell. Keep the first
 * failure visible when the backend follows it with an exit notification. */
export function terminalEvents(
  write: (data: Uint8Array) => void,
  status: (value: SessionStatus) => void,
  error: (message?: string) => void,
) {
  let ended = false
  return {
    ended: () => ended,
    receive(event: TerminalEvent) {
      if (ended) return
      switch (event.type) {
        case 'ready':
          status('open')
          break
        case 'data':
          write(new Uint8Array(event.data))
          status('open')
          break
        case 'error':
          ended = true
          error(event.message)
          status('error')
          break
        case 'exit':
          ended = true
          status('closed')
          break
      }
    },
  }
}
