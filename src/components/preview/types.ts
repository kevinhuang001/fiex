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
    name?: string
    mime?: string
    media?: 'video' | 'audio'
    message?: string
    sessionId?: string
    pluginId?: string
    contributionId?: string
    timeoutMs?: number
    context?: unknown
    state?: unknown
    size?: number
    extension?: string
  }
}

export interface PluginLoadingPage {
  pluginId: string
  /** Bridge-only session for the loading document; Core has no preview session yet. */
  sessionId: string
  src: string
}
