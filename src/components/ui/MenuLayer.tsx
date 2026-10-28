import { createPortal } from 'react-dom'
import type { ReactNode } from 'react'

/** Popup menus render at the document root.
 *
 * The sidebar and the terminal panel each create their own stacking context, so
 * a menu nested inside one of them is painted relative to that context and can
 * end up under the other panel. Portaling keeps every menu above the app shell;
 * pointer handlers still use the same element refs, so dismissal is unchanged. */
export function MenuLayer({ children }: { children: ReactNode }) {
  if (typeof document === 'undefined') return null
  return createPortal(children, document.body)
}
