import { requestDirectoryRefresh } from './listing'

type TerminalTaskStatus = 'completed' | 'failed' | 'cancelled'

const EVENT_TTL_MS = 60_000
const MAX_EARLY_EVENTS = 256
const MAX_TRACKED_TASKS = 512

/** Correlates async plugin completion events with the directory that invoked
 * the action. Completion can arrive before execute returns its taskId. */
export class PluginTaskRefreshTracker {
  private readonly directories = new Map<string, string>()
  private readonly earlyTerminalEvents = new Map<
    string,
    { status: TerminalTaskStatus; expiresAt: number }
  >()

  constructor(
    private readonly refresh: (directories: string[]) => void = requestDirectoryRefresh,
    private readonly now: () => number = Date.now,
  ) {}

  register(taskId: string, directory: string) {
    if (!taskId || !directory) return
    this.prune()
    const early = this.earlyTerminalEvents.get(taskId)
    if (early) {
      this.earlyTerminalEvents.delete(taskId)
      if (early.status === 'completed') this.refresh([directory])
      return
    }
    this.directories.set(taskId, directory)
    while (this.directories.size > MAX_TRACKED_TASKS) {
      const oldest = this.directories.keys().next().value
      if (oldest === undefined) break
      this.directories.delete(oldest)
    }
  }

  settle(taskId: string, status: string) {
    if (!this.isTerminal(status)) return
    this.prune()
    const directory = this.directories.get(taskId)
    if (directory) {
      this.directories.delete(taskId)
      if (status === 'completed') this.refresh([directory])
      return
    }
    this.earlyTerminalEvents.delete(taskId)
    this.earlyTerminalEvents.set(taskId, {
      status,
      expiresAt: this.now() + EVENT_TTL_MS,
    })
    while (this.earlyTerminalEvents.size > MAX_EARLY_EVENTS) {
      const oldest = this.earlyTerminalEvents.keys().next().value
      if (oldest === undefined) break
      this.earlyTerminalEvents.delete(oldest)
    }
  }

  private isTerminal(status: string): status is TerminalTaskStatus {
    return status === 'completed' || status === 'failed' || status === 'cancelled'
  }

  private prune() {
    const now = this.now()
    for (const [taskId, event] of this.earlyTerminalEvents) {
      if (event.expiresAt <= now) this.earlyTerminalEvents.delete(taskId)
    }
  }
}

export const pluginTaskRefreshTracker = new PluginTaskRefreshTracker()
