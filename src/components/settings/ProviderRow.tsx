import { t } from '@/i18n'
import { ArrowDown, ArrowUp, GripVertical, LockKeyhole, Puzzle, ShieldCheck } from 'lucide-react'
import type { ProviderEntry } from './ProviderSettings'

export function ProviderRow({
  entry,
  index,
  last,
  enabled,
  busy,
  dragging,
  onDrag,
  onDragEnd,
  onDrop,
  onMove,
  onToggle,
}: {
  entry: ProviderEntry
  index: number
  last: boolean
  enabled: boolean
  busy: boolean
  dragging: boolean
  onDrag(): void
  onDragEnd(): void
  onDrop(): void
  onMove(delta: number): void
  onToggle(): void
}) {
  return (
    <article
      className={`provider-choice ${enabled ? '' : 'is-disabled'} ${dragging ? 'is-dragging' : ''} ${entry.locked ? 'is-fallback' : ''}`}
      onDragOver={(event) => {
        if (!entry.locked && !busy) event.preventDefault()
      }}
      onDrop={(event) => {
        event.preventDefault()
        if (!entry.locked && !busy) onDrop()
      }}
    >
      <span
        className="provider-grip"
        draggable={!entry.locked && !busy}
        onDragStart={(event) => {
          event.dataTransfer.effectAllowed = 'move'
          event.dataTransfer.setData('text/plain', entry.id)
          onDrag()
        }}
        onDragEnd={onDragEnd}
        aria-hidden="true"
      >
        {entry.locked ? <LockKeyhole size={14} /> : <GripVertical size={17} />}
      </span>
      <div className={`provider-choice-icon ${entry.builtin ? 'builtin' : ''}`}>
        {entry.builtin ? <ShieldCheck size={19} /> : <Puzzle size={19} />}
      </div>
      <div className="provider-choice-copy">
        <div>
          <strong>{entry.name}</strong>
          <span className="provider-choice-badge">
            {entry.locked ? t('始终可用') : entry.builtin ? t('内置') : t('插件')}
          </span>
          {entry.available === false && (
            <span className="provider-choice-badge unavailable">{t('不可用')}</span>
          )}
        </div>
        <small>
          {entry.locked
            ? t('其他提供程序不可用时自动接替')
            : entry.modes?.length
              ? entry.modes
                  .map((mode) => (mode === 'content' ? t('文件内容') : t('文件名')))
                  .join(' · ')
              : enabled
                ? t('已启用')
                : t('已停用')}
        </small>
      </div>
      {!entry.locked && (
        <div className="provider-move-actions">
          <button
            className="icon-button"
            aria-label={t('上移 {0}', entry.name)}
            disabled={busy || index === 0}
            onClick={() => onMove(-1)}
          >
            <ArrowUp size={14} />
          </button>
          <button
            className="icon-button"
            aria-label={t('下移 {0}', entry.name)}
            disabled={busy || last}
            onClick={() => onMove(1)}
          >
            <ArrowDown size={14} />
          </button>
        </div>
      )}
      <button
        className="provider-switch"
        role="switch"
        aria-label={t('启用 {0}', entry.name)}
        aria-checked={entry.locked || enabled}
        disabled={busy || !!entry.locked}
        onClick={onToggle}
      >
        <span />
      </button>
    </article>
  )
}
