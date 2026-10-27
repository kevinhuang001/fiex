/** Portal target inside the scaled app shell, so dialogs and menus follow the
 *  browser-demo font-size zoom. Falls back to the document body on first mount. */
export function overlayRoot(): HTMLElement | undefined {
  if (typeof document === 'undefined') return undefined
  return document.getElementById('fiex-overlay-root') ?? undefined
}
