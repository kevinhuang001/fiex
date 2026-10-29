import type { Explorer } from '@/hooks/useExplorer'
import { call, native } from '@/lib/api'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect, useState } from 'react'
import type { ConflictState, Transfer } from './types'

/** Native drag-and-drop: dropping onto a sidebar bookmark or the workspace
 *  keeps a reference, dropping onto a folder (or the pane itself) transfers. */
export function useDropTargets({
  x,
  conflict,
  transfer,
  bookmarkPaths,
  addWorkspace,
}: {
  x: Explorer
  conflict: ConflictState
  transfer: Transfer
  bookmarkPaths: (paths: string[]) => Promise<void>
  addWorkspace: (paths: string[]) => Promise<void>
}) {
  const [dragging, setDragging] = useState(false)
  useEffect(() => {
    if (!native) return
    let active = true
    let dispose: (() => void) | undefined
    type DropPlan = { paths: string[]; moving: boolean; references: unknown[] }
    let origin: { key: string; pending: Promise<DropPlan | null> } | null = null
    const key = (paths: string[]) => [...paths].sort().join('\0')
    void getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        void call('ci_log', {
          message: `drop event ${event.payload.type} ${
            'paths' in event.payload ? JSON.stringify(event.payload.paths) : ''
          } ${'position' in event.payload ? JSON.stringify(event.payload.position) : ''}`,
        }).catch(() => {})
        if (event.payload.type === 'enter') {
          const paths = event.payload.paths
          origin = {
            key: key(paths),
            pending: call<DropPlan | null>('internal_drop', { paths, consume: false }).catch(
              () => null,
            ),
          }
          setDragging(true)
        } else if (event.payload.type === 'leave') {
          origin = null
          setDragging(false)
        } else if (event.payload.type === 'drop') {
          setDragging(false)
          const { paths, position } = event.payload
          const captured = origin?.key === key(paths) ? origin.pending : null
          origin = null
          void (async () => {
            const current = await call<DropPlan | null>('internal_drop', {
              paths,
              consume: true,
              captured: await captured,
            })
            const drop = current || { paths, moving: false }
            const scale = await getCurrentWindow().scaleFactor()
            const element = document.elementFromPoint(position.x / scale, position.y / scale)
            if (element?.closest('[data-bookmark-drop]')) {
              await bookmarkPaths(drop.paths)
              return
            }
            if (element?.closest('[data-workspace-drop]')) {
              await addWorkspace(drop.paths)
              return
            }
            const folder = element?.closest<HTMLElement>('[data-drop-folder]')?.dataset.dropFolder
            transfer(drop.paths, folder || x.path, drop.moving)
          })().catch((error) => x.setError(String(error)))
        }
      })
      .then((fn) => {
        if (active) dispose = fn
        else fn()
      })
    return () => {
      active = false
      dispose?.()
    }
  }, [x.path, conflict, x.directoryReadOnly])
  return { dragging }
}
