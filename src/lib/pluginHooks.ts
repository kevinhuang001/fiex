import { call } from './api'
import type { Entry } from './types'

export interface BeforeOpenOutcome {
  handled: boolean
}

/** Core owns provider matching, priority, consent and the scoped event.
 * Hooks may consume the open or let Core continue.
 * A broken hook must never stop the user from opening the file. */
export async function runBeforeOpenHooks(entry: Entry): Promise<BeforeOpenOutcome> {
  try {
    const result = await call<BeforeOpenOutcome>('plugin_before_open', { path: entry.path })
    return result?.handled ? { handled: true } : { handled: false }
  } catch {
    return { handled: false }
  }
}
