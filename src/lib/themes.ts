import { call, native } from './api'
export interface Theme {
  id: string
  name: string
  mode: 'light' | 'dark'
  tokens: Record<string, string>
}
let applied: string[] = []
export async function installedThemes(): Promise<Theme[]> {
  if (native) return call<Theme[]>('plugin_themes')
  // The browser demo has no plugin host. In development the Vite server mirrors
  // the sibling official theme packages so they stay previewable on the web;
  // production browser builds have no plugins and no fallback.
  if (!import.meta.env.DEV) return []
  const response = await fetch('/__fiex_theme/index.json').catch(() => null)
  return response?.ok ? ((await response.json()) as Theme[]) : []
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
  'radius-row',
  'radius-tab',
  'radius-menu',
  'radius-submenu',
  'sidebar-background',
  'sidebar-line',
  'toolbar-background',
  'toolbar-line',
  'tabbar-background',
  'tab-background',
  'tab-active-background',
  'tab-active-line',
  'row-alt-background',
  'row-hover-background',
  'row-selected-background',
  'row-selected-line',
  'row-selected-text',
  'statusbar-background',
  'menu-background',
  'context-menu-background',
  'menu-line',
  'input-background',
  'input-line',
  'control-hover-background',
  'segmented-background',
  'segmented-active-background',
  'font-family',
]
