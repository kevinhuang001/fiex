export interface ResolvedPreview {
  providerId: string
  provider: string
  priority: number
  view: {
    kind: 'html' | 'text' | 'image' | 'media' | 'pdf' | 'fallback'
    html?: string
    entry?: string
    text?: string
    truncated?: boolean
    path?: string
    maxPages?: number
    name?: string
    mime?: string
    media?: 'video' | 'audio'
    message?: string
    sessionId?: string
    pluginId?: string
    contributionId?: string
    timeoutMs?: number
    context?: unknown
  }
}

export interface PluginLoadingPage {
  pluginId: string
  /** Bridge-only session for the loading document; Core has no preview session yet. */
  sessionId: string
  html?: string
  src?: string
}
