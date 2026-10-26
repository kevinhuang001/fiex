import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useState } from 'react'
import { ChevronRight } from 'lucide-react'
import { BrowserColumn } from '../BrowserColumn'
import { FileIcon } from '../../FileIcon'
import { TagDots } from '../../TagDots'
import { NameCell, rowVars, SyncBadge, Thumbnail } from './shared'
import { call } from '@/lib/api'
import { kind, parent, size } from '@/lib/files'
import type { Entry, Listing } from '@/lib/types'
import type { Props, SharedRow } from './shared'
import type { KeyboardEvent } from 'react'

/** Finder-style column browser: parent, current folder, selection. */
export function ColumnsView({
  explorer: x,
  prefs,
  onOpen,
  onPreview,
  shared,
  onKeyDown,
}: Pick<Props, 'explorer' | 'prefs' | 'onOpen' | 'onPreview'> & {
  shared: SharedRow
  onKeyDown: (e: KeyboardEvent) => void
}) {
  const selectedPaths = useMemo(() => new Set(x.selected), [x.selected, getLocale()])
  const [left, setLeft] = useState<Entry[]>([])
  const [right, setRight] = useState<Entry[]>([])
  const [error, setError] = useState('')
  const selected = x.entries.find((e) => e.path === x.selected[0])
  useEffect(() => {
    let active = true
    void call<Listing>('list_directory', {
      path: parent(x.path),
      hidden: prefs.hidden,
      ignores: prefs.displayIgnore,
    })
      .then((l) => {
        if (active) setLeft(l.entries)
      })
      .catch(() => setLeft([]))
    return () => {
      active = false
    }
  }, [x.path, prefs.hidden, prefs.displayIgnore])
  useEffect(() => {
    let active = true
    setRight([])
    setError('')
    if (selected?.isDir)
      void call<Listing>('list_directory', {
        path: selected.path,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
      })
        .then((l) => {
          if (active) setRight(l.entries)
        })
        .catch((e) => {
          if (active) setError(String(e))
        })
    return () => {
      active = false
    }
  }, [selected?.path, selected?.isDir, prefs.hidden, prefs.displayIgnore])
  const column = (files: Entry[], current: boolean) => (
    <BrowserColumn
      entries={files}
      prefs={prefs}
      target={current ? x.selected.at(-1) : x.path}
      render={(e) => {
        const decoration = x.decorations?.[e.path]
        const rowDecor = rowVars(decoration)
        return (
          <div
            key={e.path}
            className={`column-row ${selectedPaths.has(e.path) || e.path === x.path ? 'selected' : ''}${rowDecor.className}`}
            style={rowDecor.style}
            {...(current
              ? shared(e)
              : {
                  onClick: () => (e.isDir ? x.navigate(e.path) : onPreview(e)),
                  onDoubleClick: () => onOpen(e),
                })}
            data-drop-folder={e.isDir ? e.path : undefined}
          >
            <FileIcon entry={e} size={24} />
            <NameCell explorer={x} entry={e} className="column-label" />
            <TagDots tags={e.tags} size={6} />
            <SyncBadge sync={e.sync} />
            {e.isDir && <ChevronRight size={13} />}
          </div>
        )
      }}
    />
  )
  return (
    <div
      className="columns-view"
      role="grid"
      aria-label={t('文件')}
      tabIndex={0}
      onKeyDown={onKeyDown}
    >
      {column(left, false)}
      {column(x.entries, true)}
      {selected?.isDir ? (
        <>
          {column(right, false)}
          {error && <p className="error">{error}</p>}
        </>
      ) : selected ? (
        <div className="column-detail" onDoubleClick={() => onPreview(selected)}>
          <Thumbnail entry={selected} size={100} />
          <strong>{selected.name}</strong>
          <span>
            {kind(selected)} · {size(selected.size)}
          </span>
          <button onClick={() => onPreview(selected)}>{t('快速查看')}</button>
        </div>
      ) : (
        <div className="column-detail muted" />
      )}
    </div>
  )
}
