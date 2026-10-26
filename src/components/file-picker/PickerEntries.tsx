import { t } from '@/i18n'
import type { MouseEvent as ReactMouseEvent } from 'react'
import { FileIcon } from '@/components/FileIcon'
import { date, size } from '@/lib/files'
import type { Entry } from '@/lib/types'

import type { PickerGroup } from './usePicker'

export function PickerEntries({
  view,
  error,
  visible,
  grouped,
  selected,
  onToggle,
  onOpen,
  onContextMenu,
}: {
  view: 'list' | 'grid'
  error: string
  visible: Entry[]
  grouped: PickerGroup[]
  selected: string[]
  onToggle(entry: Entry, multi: boolean): void
  onOpen(entry: Entry): void
  onContextMenu(event: ReactMouseEvent, entry?: Entry): void
}) {
  return (
    <div
      className={`file-picker-entries ${view}`}
      role="listbox"
      aria-label={t('文件')}
      onContextMenu={(event) => onContextMenu(event)}
    >
      {error && <p className="file-picker-hint error">{error}</p>}
      {!error && !visible.length && <p className="file-picker-hint">{t('此文件夹为空。')}</p>}
      {view === 'list'
        ? grouped.map((section) => (
            <div key={section.label || 'all'}>
              {section.label && <p className="file-picker-group">{section.label}</p>}
              {section.entries.map((entry) => (
                <button
                  key={entry.path}
                  type="button"
                  role="option"
                  aria-selected={selected.includes(entry.path)}
                  className={`file-picker-row ${selected.includes(entry.path) ? 'selected' : ''}`}
                  onClick={(event) =>
                    onToggle(entry, event.metaKey || event.ctrlKey || event.shiftKey)
                  }
                  onDoubleClick={() => onOpen(entry)}
                  onContextMenu={(event) => onContextMenu(event, entry)}
                >
                  <FileIcon entry={entry} size={22} />
                  <span className="name">{entry.name}</span>
                  <span className="meta">{date(entry.modified)}</span>
                  <span className="meta size">{entry.isDir ? '—' : size(entry.size)}</span>
                </button>
              ))}
            </div>
          ))
        : visible.map((entry) => (
            <button
              key={entry.path}
              type="button"
              role="option"
              aria-selected={selected.includes(entry.path)}
              className={`file-picker-tile ${selected.includes(entry.path) ? 'selected' : ''}`}
              onClick={(event) => onToggle(entry, event.metaKey || event.ctrlKey || event.shiftKey)}
              onDoubleClick={() => onOpen(entry)}
              onContextMenu={(event) => onContextMenu(event, entry)}
            >
              <FileIcon entry={entry} size={44} />
              <span>{entry.name}</span>
            </button>
          ))}
    </div>
  )
}
