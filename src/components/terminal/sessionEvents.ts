export type SessionStatus = 'connecting' | 'open' | 'closed' | 'error' | 'demo'
export type TerminalEvent =
  | { type: 'ready' }
  | { type: 'data'; data: number[] }
  | { type: 'exit'; code?: number | null }
  | { type: 'error'; message?: string }

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
