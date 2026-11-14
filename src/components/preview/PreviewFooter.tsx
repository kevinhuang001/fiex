import { t } from '@/i18n'
import {
  Check,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  ExternalLink,
  Monitor,
  Puzzle,
} from 'lucide-react'
import { Button } from '../ui/button'
import { date, size } from '@/lib/files'
import type { Entry } from '@/lib/types'
import type { PreviewCandidate } from '@/lib/preview'
import type { RefObject } from 'react'

type PreviewFooterProps = {
  entry: Entry
  candidates: PreviewCandidate[]
  providerId: string | null
  providerLabel: string
  fromPlugin: boolean
  menuOpen: boolean
  sourceRef: RefObject<HTMLDivElement | null>
  onNavigate: (delta: number) => void
  onOpen: (entry: Entry) => void
  onMenuOpenChange: (open: boolean) => void
  onProviderSelect: (providerId: string) => void
}

export function PreviewFooter({
  entry,
  candidates,
  providerId,
  providerLabel,
  fromPlugin,
  menuOpen,
  sourceRef,
  onNavigate,
  onOpen,
  onMenuOpenChange,
  onProviderSelect,
}: PreviewFooterProps) {
  return (
    <footer className="preview-footer">
      <Button variant="ghost" size="icon" title={t('上一个')} onClick={() => onNavigate(-1)}>
        <ChevronLeft />
      </Button>
      <Button variant="ghost" size="icon" title={t('下一个')} onClick={() => onNavigate(1)}>
        <ChevronRight />
      </Button>
      <span>
        {entry.isDir ? t('文件夹') : size(entry.size)} · {date(entry.modified)}
      </span>
      {candidates.length > 0 && (
        <div className="preview-source" ref={sourceRef}>
          <button
            type="button"
            className="preview-source-button"
            aria-haspopup="listbox"
            aria-expanded={menuOpen}
            title={t('切换预览方式')}
            onClick={() => onMenuOpenChange(!menuOpen)}
          >
            {fromPlugin ? <Puzzle size={13} /> : <Monitor size={13} />}
            <span>{providerLabel}</span>
            <ChevronUp size={12} style={{ transform: menuOpen ? undefined : 'rotate(180deg)' }} />
          </button>
          {menuOpen && (
            <div className="preview-source-menu" role="listbox" aria-label={t('预览方式')}>
              {candidates.map((candidate) => (
                <button
                  type="button"
                  role="option"
                  aria-selected={providerId === candidate.id}
                  className={'preview-source-item' + (providerId === candidate.id ? ' active' : '')}
                  key={candidate.id}
                  onClick={() => onProviderSelect(candidate.id)}
                >
                  {candidate.id.startsWith('plugin:') ? (
                    <Puzzle size={14} />
                  ) : (
                    <Monitor size={14} />
                  )}
                  <span className="preview-source-copy">
                    <span className="preview-source-title">
                      {candidate.id.startsWith('plugin:') ? candidate.provider : t('Fiex 内置')}
                    </span>
                  </span>
                  {providerId === candidate.id && <Check size={14} />}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
      <Button variant="secondary" onClick={() => onOpen(entry)}>
        {t('打开')}
        <ExternalLink />
      </Button>
    </footer>
  )
}
