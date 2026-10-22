import { useEffect, useRef, useState, type PointerEvent } from 'react'
import { createPortal } from 'react-dom'
import type { AppController } from '@/hooks/useAppController'
import { Columns2, Folder, Plus, X } from 'lucide-react'
import { WindowControls } from '@/components/WindowControls'

type TabBox = { left: number; top: number; width: number; height: number; center: number }
type DragOrigin = { pointerX: number; grab: number; rects: TabBox[]; gap: number }
type DragState = {
  from: number
  to: number
  left: number
  top: number
  width: number
  height: number
  gap: number
  settling: boolean
}

export function ExplorerTabs({
  left,
  panePaths,
  displayName,
  switchWorkspaceTab,
  closeWorkspaceTab,
  newWorkspaceTab,
  moveWorkspaceTab,
  control,
  platform,
}: Pick<
  AppController,
  | 'left'
  | 'panePaths'
  | 'displayName'
  | 'switchWorkspaceTab'
  | 'closeWorkspaceTab'
  | 'newWorkspaceTab'
  | 'moveWorkspaceTab'
  | 'control'
  | 'platform'
>) {
  const [drag, setDrag] = useState<DragState | null>(null)
  const dragged = useRef(false)
  const origin = useRef<DragOrigin | null>(null)
  const detach = useRef<(() => void) | null>(null)
  const dropTimer = useRef<number | null>(null)
  const strip = useRef<HTMLDivElement>(null)
  // With enough tabs the strip scrolls; keep the active one reachable after
  // switching or opening a tab.
  useEffect(() => {
    strip.current
      ?.querySelector<HTMLElement>('.tab.active')
      ?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }, [left.tab, left.tabs.length])
  useEffect(
    () => () => {
      detach.current?.()
      if (dropTimer.current !== null) window.clearTimeout(dropTimer.current)
    },
    [],
  )
  const tabBody = (path: string, index: number) => (
    <>
      {panePaths[index] ? <Columns2 size={14} /> : <Folder size={13} />}
      <span>
        {displayName(path)}
        {panePaths[index] ? ` · ${displayName(panePaths[index])}` : ''}
      </span>
    </>
  )
  const beginDrag = (event: PointerEvent, from: number) => {
    if (event.button !== 0 || (event.target as Element).closest('[aria-label="关闭标签页"]')) return
    if (drag || !strip.current) return
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    const rects = Array.from(strip.current.querySelectorAll<HTMLElement>('.tab')).map((element) => {
      const box = element.getBoundingClientRect()
      return {
        left: box.left,
        top: box.top,
        width: box.width,
        height: box.height,
        center: box.left + box.width / 2,
      }
    })
    const gap = rects.length > 1 ? Math.max(0, rects[1].left - (rects[0].left + rects[0].width)) : 6
    origin.current = { pointerX: event.clientX, grab: event.clientX - rect.left, rects, gap }
    dragged.current = false
    let started = false
    let to = from
    const move = (next: globalThis.PointerEvent) => {
      const data = origin.current
      if (!data) return
      if (!started) {
        if (Math.abs(next.clientX - data.pointerX) < 5) return
        started = true
        dragged.current = true
      }
      next.preventDefault()
      const box = data.rects[from]
      const left = next.clientX - data.grab
      // Insert after every tab whose centre the dragged tab has passed. The
      // source tab is excluded so the count is the final array index.
      const center = left + box.width / 2
      let nextTo = 0
      for (let index = 0; index < data.rects.length; index++) {
        if (index !== from && center > data.rects[index].center) nextTo++
      }
      to = nextTo
      setDrag({
        from,
        to,
        left,
        top: box.top,
        width: box.width,
        height: box.height,
        gap: data.gap,
        settling: false,
      })
    }
    const finish = (next: globalThis.PointerEvent) => {
      detach.current?.()
      const data = origin.current
      origin.current = null
      if (!started || next.type === 'pointercancel' || !data) {
        setDrag(null)
        setTimeout(() => {
          dragged.current = false
        }, 0)
        return
      }
      const target = to
      setDrag((current) =>
        current ? { ...current, left: data.rects[target].left, settling: true } : current,
      )
      dropTimer.current = window.setTimeout(() => {
        dropTimer.current = null
        setDrag(null)
        if (target !== from) moveWorkspaceTab(from, target)
        setTimeout(() => {
          dragged.current = false
        }, 0)
      }, 170)
    }
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      detach.current = null
    }
    detach.current = cleanup
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
  }
  // While a tab is lifted the other tabs slide by the width of the vacated
  // slot, opening a gap under the pointer for the dragged tab to land in.
  const shift = (index: number) => {
    if (!drag || index === drag.from) return undefined
    const distance = drag.width + drag.gap
    if (drag.from < drag.to && index > drag.from && index <= drag.to)
      return `translate3d(-${distance}px, 0, 0)`
    if (drag.to < drag.from && index >= drag.to && index < drag.from)
      return `translate3d(${distance}px, 0, 0)`
    return undefined
  }
  const lifted = drag && left.tabs[drag.from] ? drag : null
  return (
    <div className="tabs" data-tauri-drag-region="deep">
      <div
        className={`tabs-scroll ${drag ? 'tabs-dragging' : ''}`}
        role="tablist"
        aria-label="标签页"
        ref={strip}
      >
        {left.tabs.map((path, i) => (
          <div
            className={`tab ${i === left.tab ? 'active' : ''} ${drag?.from === i ? 'tab-source' : ''}`}
            key={i}
            data-tab-index={i}
            data-tauri-drag-region="false"
            style={{ transform: shift(i) }}
            onPointerDown={(event) => beginDrag(event, i)}
          >
            <button
              role="tab"
              title={panePaths[i] ? `${path} · ${panePaths[i]}` : path}
              aria-selected={i === left.tab}
              onClick={() => {
                if (!dragged.current) switchWorkspaceTab(i)
              }}
            >
              {tabBody(path, i)}
            </button>
            {left.tabs.length > 1 && (
              <button aria-label="关闭标签页" onClick={() => closeWorkspaceTab(i)}>
                <X size={12} />
              </button>
            )}
          </div>
        ))}
      </div>
      <button
        className="icon-button tabs-new"
        title="新建标签页"
        aria-label="新建标签页"
        onClick={newWorkspaceTab}
      >
        <Plus size={18} />
      </button>
      <div className="tabs-spacer" data-tauri-drag-region />
      {(platform === 'windows' || platform === 'linux') && <WindowControls control={control} />}
      {lifted &&
        createPortal(
          <div
            className={`tab tab-ghost ${lifted.from === left.tab ? 'active' : ''} ${lifted.settling ? 'settling' : ''}`}
            style={{
              width: lifted.width,
              height: lifted.height,
              transform: `translate3d(${lifted.left}px, ${lifted.top}px, 0) scale(1.02)`,
            }}
            aria-hidden
          >
            <button tabIndex={-1}>{tabBody(left.tabs[lifted.from], lifted.from)}</button>
            {left.tabs.length > 1 && (
              <button tabIndex={-1}>
                <X size={12} />
              </button>
            )}
          </div>,
          document.body,
        )}
    </div>
  )
}
