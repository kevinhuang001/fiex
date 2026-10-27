import { t } from '@/i18n'
import { FileIcon } from '../../FileIcon'
import { DecoratedValue, NameCell, rowVars } from './shared'
import { date, kind, size } from '@/lib/files'
import type { Entry } from '@/lib/types'
import type { Explorer } from '@/hooks/useExplorer'
import type { SharedRow } from './shared'
import type { VirtualItem } from '@tanstack/react-virtual'

export interface FlatRow {
  heading: string
  entry: Entry | null
}

/** Virtualized list with a fixed header row. */
export function ListView({
  explorer,
  flat,
  rows,
  totalSize,
  selectedPaths,
  shared,
}: {
  explorer: Explorer
  flat: FlatRow[]
  rows: VirtualItem[]
  totalSize: number
  selectedPaths: Set<string>
  shared: SharedRow
}) {
  return (
    <>
      <div className="list-header">
        <span>{t('名称')}</span>
        <span>{t('修改时间')}</span>
        <span>{t('大小')}</span>
        <span>{t('种类')}</span>
      </div>
      <div style={{ height: totalSize, position: 'relative' }}>
        {rows.map((row) => {
          const item = flat[row.index]
          const decoration = item.entry ? explorer.decorations?.[item.entry.path] : undefined
          const rowDecor = rowVars(decoration)
          return (
            <div
              key={item.entry?.path || item.heading}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: row.size,
                transform: `translateY(${row.start}px)`,
              }}
            >
              {item.entry ? (
                <div
                  role="row"
                  className={`file-row ${selectedPaths.has(item.entry.path) ? 'selected' : ''}${rowDecor.className}`}
                  style={rowDecor.style}
                  {...shared(item.entry)}
                >
                  <span className="file-name">
                    <FileIcon entry={item.entry} size={25} />
                    <NameCell explorer={explorer} entry={item.entry} className="file-label" />
                  </span>
                  <DecoratedValue decoration={decoration} target="modified">
                    {date(item.entry.modified)}
                  </DecoratedValue>
                  <DecoratedValue decoration={decoration} target="size">
                    {item.entry.isDir ? '—' : size(item.entry.size)}
                  </DecoratedValue>
                  <DecoratedValue decoration={decoration} target="kind">
                    {kind(item.entry)}
                  </DecoratedValue>
                </div>
              ) : (
                <div className="group-heading">{item.heading}</div>
              )}
            </div>
          )
        })}
      </div>
    </>
  )
}
