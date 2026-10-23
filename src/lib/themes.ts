import { call, native } from './api'
export interface Theme {
  id: string
  name: string
  mode: 'light' | 'dark'
  tokens: Record<string, string>
}
let applied: string[] = []
export async function installedThemes(): Promise<Theme[]> {
  return native ? call<Theme[]>('plugin_themes') : []
}
export function applyTheme(theme: Theme | undefined, mode: 'light' | 'dark') {
  const root = document.documentElement
  for (const key of applied) root.style.removeProperty('--' + key)
  applied = []
  root.classList.toggle('dark', theme?.mode === 'dark' || (!theme && mode === 'dark'))
  root.dataset.theme = theme?.id || mode
  root.style.colorScheme = theme?.mode || mode
  for (const [key, value] of Object.entries(theme?.tokens || {})) {
    if (!/^[a-z][a-z0-9-]*$/.test(key) || /[;{}]|url/i.test(value)) continue
    root.style.setProperty('--' + key, value)
    applied.push(key)
  }
  window.dispatchEvent(new Event('fiex:theme-changed'))
}
export function themeSnapshot() {
  const style = getComputedStyle(document.documentElement)
  const tokens: Record<string, string> = {}
  for (const key of [...new Set([...THEME_TOKENS, ...applied])])
    tokens[key] = style.getPropertyValue('--' + key).trim()
  tokens['panel-bg'] = tokens.canvas
  return { mode: document.documentElement.classList.contains('dark') ? 'dark' : 'light', tokens }
}
export const THEME_TOKENS = [
  'text',
  'muted',
  'surface',
  'canvas',
  'hover',
  'line',
  'accent',
  'selected',
  'glass',
  'shadow',
  'overlay',
  'danger',
  'success',
  'warning',
  'on-accent',
  'radius-control',
  'radius-panel',
  'sidebar-background',
  'toolbar-background',
  'font-family',
]
