import { useEffect, useMemo, useRef } from 'react'
import type { ReactNode } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { sortEntries } from '@/lib/files'
import type { Entry, Preferences } from '@/lib/types'

export function BrowserColumn({
  entries,
  prefs,
  target,
  render,
}: {
  entries: Entry[]
  prefs: Preferences
  target?: string
  render: (entry: Entry) => ReactNode
}) {
  const host = useRef<HTMLDivElement>(null)
  const sorted = useMemo(
    () => sortEntries(entries, prefs.sort, prefs.descending),
    [entries, prefs.sort, prefs.descending],
  )
  const virtual = useVirtualizer({
    count: sorted.length,
    getScrollElement: () => host.current,
    estimateSize: () => 36,
    overscan: 8,
  })
  useEffect(() => {
    if (!target || sorted.length <= 200) return
    const index = sorted.findIndex((entry) => entry.path === target)
    if (index >= 0) virtual.scrollToIndex(index, { align: 'auto' })
  }, [target, sorted, virtual])
  return (
    <div className="browser-column" ref={host}>
      {sorted.length <= 200 ? (
        sorted.map(render)
      ) : (
        <div style={{ height: virtual.getTotalSize(), position: 'relative' }}>
          {virtual.getVirtualItems().map((row) => (
            <div
              key={sorted[row.index].path}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: row.size,
                transform: `translateY(${row.start}px)`,
              }}
            >
              {render(sorted[row.index])}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
