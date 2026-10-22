import { getCurrentWindow } from '@tauri-apps/api/window'
import { Copy, Minus, Square, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { native } from '@/lib/api'

/**
 * Caption buttons for the frameless Windows/Linux window. macOS keeps the
 * native traffic lights, so this is only mounted on the other platforms.
 */
export function WindowControls({
  control,
}: {
  control: (action: 'close' | 'minimize' | 'toggleMaximize') => void
}) {
  const [maximized, setMaximized] = useState(false)
  useEffect(() => {
    if (!native) return
    let disposed = false
    let unlisten: (() => void) | undefined
    const window = getCurrentWindow()
    const sync = () => {
      void window
        .isMaximized()
        .then((value) => {
          if (!disposed) setMaximized(value)
        })
        .catch(() => {})
    }
    sync()
    void window
      .onResized(sync)
      .then((fn) => {
        if (disposed) fn()
        else unlisten = fn
      })
      .catch(() => {})
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])
  return (
    <div className="window-controls" data-tauri-drag-region="false">
      <button title="最小化" aria-label="最小化窗口" onClick={() => control('minimize')}>
        <Minus size={15} strokeWidth={1.6} />
      </button>
      <button
        title={maximized ? '向下还原' : '最大化'}
        aria-label={maximized ? '还原窗口' : '最大化窗口'}
        onClick={() => control('toggleMaximize')}
      >
        {maximized ? <Copy size={13} strokeWidth={1.6} /> : <Square size={12} strokeWidth={1.6} />}
      </button>
      <button
        className="window-close"
        title="关闭窗口"
        aria-label="关闭窗口"
        onClick={() => control('close')}
      >
        <X size={16} strokeWidth={1.6} />
      </button>
    </div>
  )
}
