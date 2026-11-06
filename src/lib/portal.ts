/** Portal target inside the application shell. Falls back to the document body
 * on first mount. */
export function overlayRoot(): HTMLElement | undefined {
  if (typeof document === 'undefined') return undefined
  return document.getElementById('fiex-overlay-root') ?? undefined
}
