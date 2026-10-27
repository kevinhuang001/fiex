/** Current browser-demo UI zoom (1 in the desktop app, where the webview
 *  handles zoom natively). Pointer deltas are measured in visual pixels, so
 *  pointer-driven sizes must be divided by this before writing CSS pixels. */
export function uiScale(): number {
  if (typeof document === 'undefined') return 1
  const value = Number.parseFloat(
    getComputedStyle(document.documentElement).getPropertyValue('--ui-scale'),
  )
  return Number.isFinite(value) && value > 0 ? value : 1
}
