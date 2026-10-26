import { useCallback, useEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'

export interface PluginPanelRequest {
  pluginId: string
  panelId: string
  title: string
  selection: string[]
  directory: string
  icon?: string
  anchor?: { x: number; y: number }
  source?: 'toolbar' | 'contextMenu' | 'preview'
  size?: { width: number; height: number }
  /** A package asset resolved and checked by Core after a dynamic menu action. */
  assetPath?: string
  /** Manifest-negotiated bridge/request budget from `extensions.ui.timeoutMs`. */
  timeoutMs?: number
  /** The plugin declares a create capability, so naming the current folder in
   * an invocation may grant that folder as the operation's output root. */
  allowCreate?: boolean
  /** Set when this panel renders a file preview for a plugin previewer. */
  preview?: { path: string; name: string; extension: string }
}
export const OPEN_PLUGIN_PANEL = 'fiex:plugin-panel:open'
export const PLUGIN_RESULT_EVENT = 'fiex:plugin-result'

const PANEL_EXIT_MS = 90
const PANEL_REVEAL_SETTLE_MS = 120
const PANEL_REVEAL_FALLBACK_MS = 260

export function panelKeyOf(request: PluginPanelRequest) {
  return JSON.stringify([request.pluginId, request.panelId, request.directory, request.selection])
}

export interface PanelSession {
  panels: PluginPanelRequest[]
  active: string
  leaving: string
  close: (key: string) => void
}

/** Keeps the open panels alive so switching between them can animate instead of
 * remounting. Only the last three are retained. */
export function usePanelSession(): PanelSession {
  const [panels, setPanels] = useState<PluginPanelRequest[]>([])
  const [active, setActive] = useState('')
  // The outgoing panel animates out before the next one mounts, so switching
  // between toolbar tools never shows two panels at once. At most one swap is
  // in flight; a newer click replaces the queued request.
  const [leaving, setLeaving] = useState('')
  const leaveTimer = useRef<number | null>(null)
  const activeKey = useRef('')
  const pending = useRef<{ request: PluginPanelRequest; key: string } | null>(null)
  // A toolbar click outside the panel closes it on pointerdown, one event
  // before the new panel opens; remember it briefly so the swap still animates.
  const closedKey = useRef('')
  const closedAt = useRef(0)
  activeKey.current = active
  const cancelExit = () => {
    if (leaveTimer.current !== null) {
      window.clearTimeout(leaveTimer.current)
      leaveTimer.current = null
    }
    pending.current = null
    setLeaving('')
  }
  useEffect(() => {
    const show = (request: PluginPanelRequest, key: string) => {
      setPanels((items) => [...items.filter((item) => panelKeyOf(item) !== key), request].slice(-3))
      setActive(key)
    }
    const open = (event: Event) => {
      const req = (event as CustomEvent<PluginPanelRequest>).detail
      if (!req?.pluginId || !req.panelId) return
      const key = panelKeyOf(req)
      const previous =
        activeKey.current || (performance.now() - closedAt.current < 150 ? closedKey.current : '')
      if (!previous || previous === key) {
        cancelExit()
        show(req, key)
        return
      }
      pending.current = { request: req, key }
      setLeaving(previous)
      if (leaveTimer.current !== null) window.clearTimeout(leaveTimer.current)
      leaveTimer.current = window.setTimeout(() => {
        leaveTimer.current = null
        const next = pending.current
        pending.current = null
        setLeaving('')
        if (next) show(next.request, next.key)
      }, PANEL_EXIT_MS)
    }
    const invalidate = () => {
      cancelExit()
      setPanels([])
      setActive('')
    }
    window.addEventListener(OPEN_PLUGIN_PANEL, open)
    window.addEventListener('fiex:plugins-changed', invalidate)
    return () => {
      window.removeEventListener(OPEN_PLUGIN_PANEL, open)
      window.removeEventListener('fiex:plugins-changed', invalidate)
      if (leaveTimer.current !== null) window.clearTimeout(leaveTimer.current)
    }
  }, [])
  const close = (key: string) => {
    closedKey.current = key
    closedAt.current = performance.now()
    cancelExit()
    setActive('')
  }
  return { panels, active, leaving, close }
}

export interface PanelReveal {
  revealed: boolean
  reveal: () => void
  settleReveal: () => void
  armRevealFallback: () => void
}

/** Owns the reveal animation state for one cached panel. A panel stays hidden
 * until its iframe has settled, so it never flashes at a stale size. */
export function usePanelReveal(active: boolean, leaving: boolean): PanelReveal {
  // The sheet stays invisible until the iframe has finished its own layout and
  // stopped asking for new sizes, so it never appears at a size it is about to
  // leave (that read as a jump the moment the panel opened).
  const [revealed, setRevealed] = useState(false)
  const revealTimer = useRef<number | null>(null)
  const reveal = useCallback(() => {
    if (revealTimer.current !== null) {
      window.clearTimeout(revealTimer.current)
      revealTimer.current = null
    }
    setRevealed(true)
  }, [])
  // Panels measure themselves and call panel.resize; a quiet stretch after the
  // last request is the signal that the layout has settled.
  const settleReveal = useCallback(() => {
    if (revealTimer.current !== null) window.clearTimeout(revealTimer.current)
    revealTimer.current = window.setTimeout(() => {
      revealTimer.current = null
      setRevealed(true)
    }, PANEL_REVEAL_SETTLE_MS)
  }, [])
  // Fallback for panels that never resize themselves; resize-driven panels
  // reveal sooner, as soon as their layout goes quiet.
  const armRevealFallback = useCallback(() => {
    revealTimer.current = window.setTimeout(() => {
      revealTimer.current = null
      setRevealed(true)
    }, PANEL_REVEAL_FALLBACK_MS)
  }, [])
  // A closed panel is laid out again from its previous size, and some panels
  // resize right after becoming visible again. Hide it until the next settle so
  // a reopen never jumps; the same arm/disarm keeps the exit animation intact.
  const wasActive = useRef(active)
  useEffect(() => {
    if (!active && !leaving) setRevealed(false)
    if (active && !wasActive.current) {
      if (revealTimer.current !== null) window.clearTimeout(revealTimer.current)
      revealTimer.current = window.setTimeout(() => {
        revealTimer.current = null
        setRevealed(true)
      }, PANEL_REVEAL_FALLBACK_MS)
    }
    wasActive.current = active
  }, [active, leaving])
  useEffect(
    () => () => {
      if (revealTimer.current !== null) window.clearTimeout(revealTimer.current)
    },
    [],
  )
  return { revealed, reveal, settleReveal, armRevealFallback }
}

/** Closes the active panel on an outside pointerdown or Escape, while leaving
 * clicks on plugin toolbar entries to the panel swap that they trigger. */
export function usePanelDismiss(
  active: boolean,
  panel: RefObject<HTMLDivElement | null>,
  onClose: () => void,
) {
  useEffect(() => {
    if (!active) return
    const outside = (event: PointerEvent) => {
      const target = event.target as Element | null
      if (panel.current?.contains(target)) return
      // A pinned plugin tool button opens the next panel on click. Closing here
      // would hide this panel for one frame and then show it again for the exit
      // animation, which reads as a double flash. The plugin menu button is a
      // normal outside click, so opening that menu dismisses the current frame.
      if (target?.closest('.plugin-tool-button')) return
      onClose()
    }
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape)
    }
  })
}
