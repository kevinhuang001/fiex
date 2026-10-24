import { useRef, useState } from 'react'
import { call, native } from '@/lib/api'
import { startFileDrag } from '@/lib/drag'
import type { Explorer } from '@/hooks/useExplorer'
import type { Entry } from '@/lib/types'
import type { Props } from './shared'
import type {
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
  RefObject,
} from 'react'

// macOS stops reporting pointer events once the cursor leaves the window while a
// button is held, so hand the drag to the OS a few pixels before the boundary.
const EDGE = 4

type DropTarget =
  | { kind: 'folder'; path: string; element: HTMLElement }
  | { kind: 'bookmark'; element: HTMLElement }
  | { kind: 'workspace'; element: HTMLElement }

function dropTargetAt(x: number, y: number): DropTarget | null {
  const element = document.elementFromPoint(x, y)
  const folder = element?.closest('[data-drop-folder]') as HTMLElement | null
  if (folder?.dataset.dropFolder)
    return { kind: 'folder', path: folder.dataset.dropFolder, element: folder }
  const bookmark = element?.closest('[data-bookmark-drop]') as HTMLElement | null
  if (bookmark) return { kind: 'bookmark', element: bookmark }
  const workspace = element?.closest('[data-workspace-drop]') as HTMLElement | null
  if (workspace) return { kind: 'workspace', element: workspace }
  return null
}

function clearDropHighlight() {
  document
    .querySelectorAll('.drop-target')
    .forEach((element) => element.classList.remove('drop-target'))
  document
    .querySelectorAll('.bookmark-drop')
    .forEach((element) => element.classList.remove('bookmark-drop'))
  document
    .querySelectorAll('.workspace-drop')
    .forEach((element) => element.classList.remove('workspace-drop'))
}

export interface BandRect {
  left: number
  top: number
  width: number
  height: number
}

/** Rubber-band selection over the empty space of a file area container. */
export function useSelectionBand({
  container,
  selected,
  setSelected,
}: {
  container: RefObject<HTMLDivElement | null>
  selected: string[]
  setSelected: (paths: string[]) => void
}) {
  const [band, setBand] = useState<BandRect | null>(null)
  const bandStart = useRef<{ x: number; y: number; base: string[]; additive: boolean } | null>(null)
  const bandDragged = useRef(false)
  const startBand = (event: ReactPointerEvent) => {
    if (event.button !== 0 || !container.current) return
    // Begin only from empty space: rows, tiles, inputs and buttons keep their
    // own drag/click behaviour.
    if ((event.target as Element).closest('[data-path],button,input,a')) return
    const additive = event.metaKey || event.ctrlKey || event.shiftKey
    bandStart.current = {
      x: event.clientX,
      y: event.clientY,
      base: additive ? [...selected] : [],
      additive,
    }
    bandDragged.current = false
    event.currentTarget.setPointerCapture(event.pointerId)
  }
  const moveBand = (event: ReactPointerEvent) => {
    const start = bandStart.current
    if (!start || !container.current) return
    const left = Math.min(start.x, event.clientX)
    const top = Math.min(start.y, event.clientY)
    const width = Math.abs(event.clientX - start.x)
    const height = Math.abs(event.clientY - start.y)
    if (width > 3 || height > 3) bandDragged.current = true
    setBand({ left, top, width, height })
    const bandRect = { left, top, right: left + width, bottom: top + height }
    const picked: string[] = []
    container.current.querySelectorAll<HTMLElement>('[data-path]').forEach((element) => {
      const rect = element.getBoundingClientRect()
      const hit =
        rect.left < bandRect.right &&
        rect.right > bandRect.left &&
        rect.top < bandRect.bottom &&
        rect.bottom > bandRect.top
      if (hit && element.dataset.path) picked.push(element.dataset.path)
    })
    const next = start.additive ? [...new Set([...start.base, ...picked])] : picked
    setSelected(next)
  }
  const endBand = () => {
    bandStart.current = null
    setBand(null)
  }
  const handleContainerClick = (event: ReactMouseEvent) => {
    // A finished rubber-band gesture must not collapse the selection.
    if (bandDragged.current) {
      bandDragged.current = false
      return
    }
    if (event.target === event.currentTarget) setSelected([])
  }
  return { band, startBand, moveBand, endBand, handleContainerClick }
}

/** Pointer drag towards a folder, bookmark, workspace or the native drag session. */
export function useFileDrag({
  explorer: x,
  selectedPaths,
  dragPaths,
  readOnly,
  onDrop,
  onBookmark,
  onWorkspace,
  onExternalDrag,
}: {
  explorer: Explorer
  selectedPaths: Set<string>
  dragPaths?: string[]
  readOnly: boolean
  onDrop: Props['onDrop']
  onBookmark: Props['onBookmark']
  onWorkspace: Props['onWorkspace']
  onExternalDrag: Props['onExternalDrag']
}) {
  const suppressClick = useRef(false)
  const [ghost, setGhost] = useState<{ paths: string[]; x: number; y: number } | null>(null)
  const beginPointerDrag = (event: ReactPointerEvent, entry: Entry) => {
    if (event.button !== 0 || x.renaming || readOnly) return
    if ((event.target as Element).closest('input,button,a')) return
    const paths =
      selectedPaths.has(entry.path) && dragPaths?.length
        ? [...new Set([...dragPaths, ...x.selected])]
        : [entry.path]
    if (!event.metaKey && !event.ctrlKey && !event.shiftKey && !selectedPaths.has(entry.path)) {
      x.setSelected([entry.path])
    }
    const startX = event.clientX
    const startY = event.clientY
    // Keep pointer events flowing once the cursor crosses the window edge:
    // without capture macOS/Windows stop reporting moves before the drag can
    // be handed over to the native drag session.
    const captureTarget = event.currentTarget as Element
    const pointerId = event.pointerId
    try {
      captureTarget.setPointerCapture(pointerId)
    } catch {
      // Older webviews may not support pointer capture; the edge check below
      // still handles the common case.
    }
    const releaseCapture = () => {
      try {
        if (captureTarget.hasPointerCapture?.(pointerId))
          captureTarget.releasePointerCapture(pointerId)
      } catch {
        // Capture is already gone.
      }
    }
    let active = false
    let nativeStarted = false
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      document.removeEventListener('mouseleave', leave)
      releaseCapture()
      document.body.classList.remove('dragging-file')
      clearDropHighlight()
      setGhost(null)
    }
    const startNative = (copyRequested = false) => {
      if (!active || nativeStarted) return
      nativeStarted = true
      cleanup()
      if (!native) return
      void call('ci_log', {
        message: `startNative active=${active} inner=${window.innerWidth}x${window.innerHeight} paths=${JSON.stringify(paths)}`,
      }).catch(() => {})
      void startFileDrag(
        paths,
        (result) => {
          onExternalDrag?.(result)
          if (result === 'Dropped') x.reload()
        },
        copyRequested,
      ).catch((error) => {
        x.setError(String(error))
      })
    }
    const leave = (event: MouseEvent) => startNative(event.altKey || event.ctrlKey)
    const move = (moveEvent: PointerEvent) => {
      if (!active) {
        if (Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) < 5) return
        active = true
        document.body.classList.add('dragging-file')
      }
      if (
        native &&
        (moveEvent.clientX <= EDGE ||
          moveEvent.clientY <= EDGE ||
          moveEvent.clientX >= window.innerWidth - EDGE ||
          moveEvent.clientY >= window.innerHeight - EDGE)
      ) {
        startNative(moveEvent.altKey || moveEvent.ctrlKey)
        return
      }
      moveEvent.preventDefault()
      setGhost({ paths, x: moveEvent.clientX, y: moveEvent.clientY })
      clearDropHighlight()
      const target = dropTargetAt(moveEvent.clientX, moveEvent.clientY)
      if (target?.kind === 'folder') target.element.classList.add('drop-target')
      else if (target?.kind === 'bookmark') target.element.classList.add('bookmark-drop')
      else if (target?.kind === 'workspace') target.element.classList.add('workspace-drop')
    }
    const finish = (upEvent: PointerEvent) => {
      cleanup()
      if (!active || nativeStarted) return
      suppressClick.current = true
      setTimeout(() => {
        suppressClick.current = false
      }, 0)
      const target = dropTargetAt(upEvent.clientX, upEvent.clientY)
      if (target?.kind === 'folder' && !paths.includes(target.path))
        onDrop(paths, target.path, !(upEvent.altKey || upEvent.ctrlKey))
      else if (target?.kind === 'bookmark') onBookmark(paths)
      else if (target?.kind === 'workspace') onWorkspace(paths)
    }
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
    document.addEventListener('mouseleave', leave)
  }
  return { ghost, beginPointerDrag, suppressClick }
}
