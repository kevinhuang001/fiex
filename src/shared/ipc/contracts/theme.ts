export interface Theme {
  id: string
  name: string
  mode: 'light' | 'dark'
  tokens: Record<string, string>
  darkTokens?: Record<string, string>
}
