import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode, RefObject } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { Entry } from '@/lib/types'
import { groups } from '@/lib/files'
import type { Group } from '@/lib/types'

/** Virtualize whole tile rows so grouping and grid alignment remain stable. */
export function VirtualFileGrid({
  entries,
  group,
  tileSize,
  rowHeight,
  scrollRef,
  target,
  render,
}: {
  entries: Entry[]
  group: Group
  tileSize: number
  rowHeight: number
  scrollRef: RefObject<HTMLDivElement | null>
  target: string | null
  render: (entry: Entry) => ReactNode
}) {
  const host = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(800)
  useEffect(() => {
    const element = host.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width))
    observer.observe(element)
    return () => observer.disconnect()
  }, [])
  const columns = Math.max(1, Math.floor((width + 8) / (tileSize + 8)))
  const rows = useMemo(
    () =>
      groups(entries, group).flatMap(([heading, files]) => {
        const result: { heading: string; files: Entry[] }[] = heading
          ? [{ heading, files: [] }]
          : []
        for (let i = 0; i < files.length; i += columns)
          result.push({ heading: '', files: files.slice(i, i + columns) })
        return result
      }),
    [entries, group, columns],
  )
  const virtual = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => (rows[index].heading ? 34 : rowHeight),
    overscan: 3,
  })
  useEffect(() => {
    virtual.measure()
  }, [rowHeight, columns, group, virtual])
  useEffect(() => {
    if (!target) return
    const index = rows.findIndex((row) => row.files.some((entry) => entry.path === target))
    if (index >= 0) virtual.scrollToIndex(index, { align: 'auto' })
  }, [target, rows, virtual])
  return (
    <div
      ref={host}
      className="virtual-file-grid"
      style={
        {
          height: virtual.getTotalSize(),
          position: 'relative',
          '--icon-size': `${rowHeight - 76}px`,
        } as React.CSSProperties
      }
    >
      {virtual.getVirtualItems().map((item) => (
        <div
          key={item.key}
          style={{
            position: 'absolute',
            left: 0,
            top: 0,
            width: '100%',
            height: item.size,
            transform: `translateY(${item.start}px)`,
          }}
        >
          {rows[item.index].heading ? (
            <div className="group-heading">{rows[item.index].heading}</div>
          ) : (
            <div
              className="icon-grid"
              style={{ gridTemplateColumns: `repeat(${columns}, minmax(0,1fr))` }}
            >
              {rows[item.index].files.map(render)}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}
