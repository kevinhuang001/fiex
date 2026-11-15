/** A preview provider returned by Core. Matching, priority and built-in
 * capability checks live in Core; the browser only applies user preferences. */
export interface PreviewCandidate {
  id: string
  provider: string
  priority: number
  /** Set by Core when the plugin declares `extensions.preview.loading`. */
  loading?: boolean
}

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
