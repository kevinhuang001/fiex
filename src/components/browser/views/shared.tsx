import { useEffect, useState } from 'react'
import { CloudCheck, CloudDownload } from 'lucide-react'
import { FileIcon } from '../../FileIcon'
import { InlineRename } from '../InlineRename'
import { asset } from '@/lib/api'
import { useDecorationIcon } from '@/lib/decorationIcons'
import { isImage } from '@/lib/files'
import type { Explorer } from '@/hooks/useExplorer'
import type {
  DecorationBadge,
  DecorationField,
  DecorationStyle,
  DecorationTarget,
  Entry,
  EntryDecoration,
  Preferences,
} from '@/lib/types'
import type { CSSProperties, MouseEvent, PointerEvent, ReactNode } from 'react'

export interface Props {
  explorer: Explorer
  prefs: Preferences
  onOpen: (e: Entry) => void
  onPreview: (e: Entry) => void
  onMenu: (e: Entry, x: number, y: number) => void
  onDrop: (paths: string[], dest: string, moving: boolean) => void
  onBookmark: (paths: string[]) => void
  onWorkspace: (paths: string[]) => void
  onExternalDrag?: (result: 'Dropped' | 'Cancelled') => void
  dragPaths?: string[]
  readOnly?: boolean
  trashPath?: string | null
}

/** Event bindings shared by every row, tile and column item. */
interface RowBindings {
  onClick: (event: MouseEvent) => void
  onDoubleClick: () => void
  onContextMenu: (event: MouseEvent) => void
  onPointerDown: (event: PointerEvent) => void
  'data-path': string
  'data-drop-folder': string | undefined
  'aria-selected': boolean
  tabIndex: number
}

export type SharedRow = (entry: Entry) => RowBindings

export function SyncBadge({ sync }: { sync: Entry['sync'] }) {
  if (!sync) return null
  const labels = {
    synced: '已缓存',
    cloud: '仅云端',
    pinned: '始终保留在本地',
    pending: '等待上传',
    conflict: '同步冲突',
  }
  const label = labels[sync]
  return (
    <span className={`sync-badge ${sync}`} title={label}>
      {sync === 'cloud' ? <CloudDownload size={11} /> : <CloudCheck size={11} />}
      <span>{label}</span>
    </span>
  )
}

function fieldOf(
  decoration: EntryDecoration | undefined,
  target: DecorationTarget,
): DecorationField | undefined {
  const field = decoration?.fields?.[target]
  if (field) return field
  if (target === 'name' && decoration) return { badges: decoration.badges, style: decoration.style }
  return undefined
}

function decorationVars(style: DecorationStyle): CSSProperties {
  return {
    '--decoration-color': style.color,
    '--decoration-weight': style.fontWeight,
    '--decoration-font-style': style.fontStyle,
    '--decoration-bg-light': style.backgroundLight,
    '--decoration-bg-dark': style.backgroundDark,
  } as CSSProperties
}

function fieldStyle(style: DecorationStyle | undefined): CSSProperties | undefined {
  if (!style) return undefined
  return {
    ...decorationVars(style),
    color: style.color,
    fontWeight: style.fontWeight,
    fontStyle: style.fontStyle,
  } as CSSProperties
}

function Badges({ badges }: { badges?: DecorationBadge[] }) {
  return (
    <>
      {badges?.map((badge) => (
        <span
          key={`${badge.text}-${badge.color}`}
          className={`decoration-badge ${badge.color.startsWith('#') ? 'custom' : badge.color}`}
          style={
            badge.color.startsWith('#')
              ? { color: badge.color, borderColor: badge.color }
              : undefined
          }
          title={badge.tooltip}
        >
          {badge.text}
        </span>
      ))}
    </>
  )
}

export function DecoratedValue({
  decoration,
  target,
  children,
}: {
  decoration: EntryDecoration | undefined
  target: Exclude<DecorationTarget, 'name' | 'row'>
  children: ReactNode
}) {
  const field = fieldOf(decoration, target)
  const style = field?.style
  const painted = Boolean(style?.backgroundLight || style?.backgroundDark)
  return (
    <span
      className={'decorated-value' + (painted ? ' decorated-value-bg' : '')}
      style={fieldStyle(style)}
    >
      {children}
      <Badges badges={field?.badges} />
    </span>
  )
}

export function rowVars(decoration: EntryDecoration | undefined): {
  className: string
  style?: CSSProperties
} {
  const style = fieldOf(decoration, 'row')?.style
  if (!style) return { className: '' }
  return {
    className: ` decorated${style.backgroundLight || style.backgroundDark ? ' decorated-bg' : ''}`,
    style: decorationVars(style),
  }
}

export function NameCell({
  explorer,
  entry,
  className,
}: {
  explorer: Explorer
  entry: Entry
  className: string
}) {
  if (explorer.renaming === entry.path)
    return (
      <InlineRename
        entry={entry}
        className={className}
        onCommit={(name) => explorer.renameTo(entry.path, name)}
        onCancel={explorer.cancelRename}
      />
    )
  const decoration = explorer.decorations?.[entry.path]
  const field = fieldOf(decoration, 'name')
  const style = fieldStyle(field?.style)
  const title = [entry.name, decoration?.description, decoration?.providers?.join(' · ')]
    .filter(Boolean)
    .join(' · ')
  const label = decoration?.label || entry.name
  return (
    <>
      <span
        className={
          className +
          (field?.style?.backgroundLight || field?.style?.backgroundDark
            ? ' decoration-name-bg'
            : '')
        }
        style={style}
        title={title}
      >
        {entry.name}
      </span>
      <Badges badges={field?.badges} />
      {label !== entry.name && <small className="decoration-label">{label}</small>}
    </>
  )
}

export function Thumbnail({ entry: e, size: s }: { entry: Entry; size: number }) {
  const [failed, setFailed] = useState(false)
  const decorationIcon = useDecorationIcon(e.path)
  useEffect(() => setFailed(false), [e.path])
  if (decorationIcon)
    return (
      <img
        className="thumbnail"
        style={{ width: s, height: s }}
        src={decorationIcon}
        alt=""
        draggable={false}
      />
    )
  if (isImage(e) && !failed && asset(e.path))
    return (
      <img
        className="thumbnail"
        style={{ width: s, height: s }}
        src={asset(e.path)}
        alt=""
        loading="lazy"
        draggable={false}
        onError={() => setFailed(true)}
      />
    )
  return <FileIcon entry={e} size={s} />
}
