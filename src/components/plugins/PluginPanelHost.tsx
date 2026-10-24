import { useEffect, useRef, useState } from 'react'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import { PanelView } from './panel/PanelView'
import { usePanelBridge } from './panel/usePanelBridge'
import {
  PLUGIN_RESULT_EVENT,
  panelKeyOf,
  usePanelDismiss,
  usePanelReveal,
  usePanelSession,
} from './panel/usePanelSession'
import type { PluginPanelRequest } from './panel/usePanelSession'

export { OPEN_PLUGIN_PANEL, PLUGIN_RESULT_EVENT } from './panel/usePanelSession'
export type { PluginPanelRequest } from './panel/usePanelSession'

/** Renders the cached plugin panels and the shared result dialog. Panels are
 * kept mounted so switching between them animates instead of reloading. */
export function PluginPanelHost() {
  const { panels, active, leaving, close } = usePanelSession()
  return (
    <>
      {panels.map((request) => {
        const key = panelKeyOf(request)
        const exiting = leaving === key
        return (
          <CachedPanel
            key={key}
            initialRequest={request}
            active={active === key && !exiting}
            leaving={exiting}
            onClose={() => close(key)}
          />
        )
      })}
      <PluginResult />
    </>
  )
}

function CachedPanel({
  initialRequest,
  active,
  leaving,
  onClose,
}: {
  initialRequest: PluginPanelRequest
  active: boolean
  leaving: boolean
  onClose: () => void
}) {
  const panelRef = useRef<HTMLDivElement>(null)
  const { revealed, reveal, settleReveal, armRevealFallback } = usePanelReveal(active, leaving)
  const { frameRef, html, sourceUrl, ready, error, size, attach } = usePanelBridge({
    request: initialRequest,
    active,
    onClose,
    onReveal: reveal,
    onSettleReveal: settleReveal,
    onRevealFallback: armRevealFallback,
  })
  usePanelDismiss(active, panelRef, onClose)
  return (
    <PanelView
      request={initialRequest}
      active={active}
      leaving={leaving}
      revealed={revealed}
      ready={ready}
      error={error}
      sourceUrl={sourceUrl}
      html={html}
      size={size}
      panelRef={panelRef}
      frameRef={frameRef}
      onClose={onClose}
      onIframeLoad={attach}
    />
  )
}

function PluginResult() {
  const [result, setResult] = useState<{ title: string; value: unknown } | null>(null)
  useEffect(() => {
    const show = (event: Event) => {
      const detail = (event as CustomEvent<{ title: string; result: unknown }>).detail
      setResult({ title: detail.title, value: detail.result })
    }
    window.addEventListener(PLUGIN_RESULT_EVENT, show)
    return () => window.removeEventListener(PLUGIN_RESULT_EVENT, show)
  }, [])
  return (
    <Modal
      open={Boolean(result)}
      title={result?.title || '插件结果'}
      onClose={() => setResult(null)}
      wide
    >
      <pre className="plugin-command-result">{result && JSON.stringify(result.value, null, 2)}</pre>
      <footer className="plugin-consent-actions">
        <Button onClick={() => setResult(null)}>关闭</Button>
      </footer>
    </Modal>
  )
}
