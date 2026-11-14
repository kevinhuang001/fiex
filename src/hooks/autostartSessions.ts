export interface AutostartReadiness {
  native: boolean
  alreadyStarted: boolean
  hasListing: boolean
  connectionsLoaded: boolean
}

export function shouldRestoreAutostartSessions(state: AutostartReadiness): boolean {
  return state.native && !state.alreadyStarted && state.hasListing && state.connectionsLoaded
}
