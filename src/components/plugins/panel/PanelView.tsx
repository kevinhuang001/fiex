import type { RefObject } from 'react'
import { X } from 'lucide-react'
import { PluginGlyph } from '../PluginGlyph'
import { MAX_PANEL_HEIGHT, MAX_PANEL_WIDTH } from './usePanelBridge'
import type { PluginPanelRequest } from './usePanelSession'

const PANEL_CSP =
  "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'\">"

export interface PanelViewProps {
  request: PluginPanelRequest
  active: boolean
  leaving: boolean
  revealed: boolean
  ready: boolean
  error: string
  sourceUrl: string
  html: string
  size: { width: number; height: number }
  panelRef: RefObject<HTMLDivElement | null>
  frameRef: RefObject<HTMLIFrameElement | null>
  onClose: () => void
  onIframeLoad: () => void
}

/** The popover chrome around one plugin iframe: title bar, loading/error states
 * and the reveal/exit animation classes. */
export function PanelView({
  request,
  active,
  leaving,
  revealed,
  ready,
  error,
  sourceUrl,
  html,
  size,
  panelRef,
  frameRef,
  onClose,
  onIframeLoad,
}: PanelViewProps) {
  const panelWidth = Math.min(MAX_PANEL_WIDTH, size.width, window.innerWidth - 40)
  return (
    <div
      ref={panelRef}
      className={`plugin-panel-popover${
        request.source === 'toolbar' ? ' toolbar-anchored' : ''
      }${revealed ? ' revealed' : ''}${leaving ? ' leaving' : ''}`}
      hidden={!active && !leaving}
      role="dialog"
      aria-label={request.title || '插件'}
      aria-busy={!ready}
      style={
        request.source === 'toolbar'
          ? {
              width: panelWidth,
              height: Math.min(
                size.height + 52,
                MAX_PANEL_HEIGHT + 52,
                window.innerHeight - 96 - 16,
              ),
              top: 96,
              left: Math.max(
                12,
                Math.min(
                  (request.anchor?.x ?? window.innerWidth - 18) - panelWidth / 2,
                  window.innerWidth - panelWidth - 12,
                ),
              ),
              right: 'auto',
            }
          : {
              width: panelWidth,
              height: Math.min(size.height + 52, MAX_PANEL_HEIGHT + 52, window.innerHeight - 40),
            }
      }
    >
      <header>
        <span className="plugin-panel-icon">
          <PluginGlyph icon={request.icon} />
        </span>
        <strong>{request.title || '插件'}</strong>
        <button
          className="icon-button plugin-panel-close"
          aria-label="关闭插件面板"
          onClick={onClose}
        >
          <X size={17} />
        </button>
      </header>
      <div className="plugin-panel-content">
        {!ready && !error && (
          <div role="status" className="plugin-panel-loading">
            正在加载插件…
          </div>
        )}
        {error ? (
          <p role="alert" className="connection-error">
            {error}
          </p>
        ) : sourceUrl ? (
          <iframe
            key="asset"
            ref={frameRef}
            title={request.title || '插件界面'}
            sandbox="allow-scripts"
            referrerPolicy="no-referrer"
            style={{ height: '100%', width: '100%', border: 0 }}
            src={sourceUrl}
            onLoad={onIframeLoad}
          />
        ) : html ? (
          <iframe
            key="srcdoc"
            ref={frameRef}
            title={request.title || '插件界面'}
            sandbox="allow-scripts"
            referrerPolicy="no-referrer"
            style={{ height: '100%', width: '100%', border: 0 }}
            srcDoc={PANEL_CSP + html}
            onLoad={onIframeLoad}
          />
        ) : null}
      </div>
    </div>
  )
}
