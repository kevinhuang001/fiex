import { t } from '@/i18n'
import { TagDots } from '../../TagDots'
import { NameCell, rowVars, SyncBadges, Thumbnail } from './shared'
import { VirtualFileGrid } from '../VirtualFileGrid'
import { groups, kind, size } from '@/lib/files'
import type { Entry, Preferences } from '@/lib/types'
import type { Explorer } from '@/hooks/useExplorer'
import type { SharedRow } from './shared'
import type { CSSProperties, RefObject } from 'react'

/** Gallery preview strip and the grouped icon/gallery grids. */
export function GridView({
  explorer,
  prefs,
  entries,
  iconSize,
  selectedPaths,
  shared,
  container,
  onPreview,
}: {
  explorer: Explorer
  prefs: Preferences
  entries: Entry[]
  iconSize: number
  selectedPaths: Set<string>
  shared: SharedRow
  container: RefObject<HTMLDivElement | null>
  onPreview: (e: Entry) => void
}) {
  return (
    <>
      {prefs.view === 'gallery' && explorer.selected.length > 0 && (
        <div
          className="gallery-preview"
          onDoubleClick={() => onPreview(entries.find((e) => e.path === explorer.selected[0])!)}
        >
          {(() => {
            const e = entries.find((e) => e.path === explorer.selected[0])
            return e ? (
              <>
                <Thumbnail entry={e} size={140} />
                <NameCell explorer={explorer} entry={e} className="gallery-name" />
                <span>
                  {kind(e)} · {e.isDir ? t('文件夹') : size(e.size)}
                </span>
                <button onClick={() => onPreview(e)}>{t('空格快速查看')}</button>
              </>
            ) : null
          })()}
        </div>
      )}
      {prefs.view === 'icons' && entries.length > 200 ? (
        <VirtualFileGrid
          entries={entries}
          group={prefs.group}
          tileSize={Math.max(96, iconSize + 38)}
          rowHeight={iconSize + 76}
          scrollRef={container}
          target={explorer.renaming || explorer.selected.at(-1) || null}
          render={(e) => {
            const decoration = explorer.decorations?.[e.path]
            const rowDecor = rowVars(decoration)
            return (
              <div
                role="gridcell"
                className={`file-tile ${selectedPaths.has(e.path) ? 'selected' : ''}${rowDecor.className}`}
                key={e.path}
                style={rowDecor.style}
                {...shared(e)}
              >
                <span className="tile-icon">
                  <Thumbnail entry={e} size={iconSize} />
                </span>
                <NameCell explorer={explorer} entry={e} className="tile-label" />
                <TagDots tags={e.tags} />
                <SyncBadges entry={e} />
              </div>
            )
          }}
        />
      ) : (
        groups(entries, prefs.group).map(([heading, files]) => (
          <section key={heading}>
            {heading && <div className="group-heading">{heading}</div>}
            <div
              className={prefs.view === 'gallery' ? 'gallery-strip' : 'icon-grid'}
              style={
                prefs.view !== 'gallery'
                  ? ({
                      '--tile-size': `${Math.max(96, iconSize + 38)}px`,
                      '--icon-size': `${iconSize}px`,
                      '--label-size': '11px',
                    } as CSSProperties)
                  : undefined
              }
              onClick={(event) => {
                if (event.target === event.currentTarget) explorer.setSelected([])
              }}
            >
              {files.map((e) => {
                const decoration = explorer.decorations?.[e.path]
                const rowDecor = rowVars(decoration)
                return (
                  <div
                    role="gridcell"
                    className={`file-tile ${selectedPaths.has(e.path) ? 'selected' : ''}${rowDecor.className}`}
                    key={e.path}
                    style={rowDecor.style}
                    {...shared(e)}
                  >
                    <span className="tile-icon">
                      <Thumbnail entry={e} size={prefs.view === 'gallery' ? 64 : iconSize} />
                    </span>
                    <NameCell explorer={explorer} entry={e} className="tile-label" />
                    <TagDots tags={e.tags} />
                    <SyncBadges entry={e} />
                  </div>
                )
              })}
            </div>
          </section>
        ))
      )}
    </>
  )
}
