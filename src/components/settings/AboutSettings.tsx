import { Button } from '@/components/ui/button'
import { call } from '@/lib/api'
import { ExternalLink, Info, Scale } from 'lucide-react'

export function AboutSettings({ onError }: { onError: (s: string) => void }) {
  const openLink = (url: string) =>
    void call('open_path', { path: url }).catch((error) => onError(String(error)))

  return (
    <>
      <div className="settings-about">
        <img src="/fiex.png" alt="" />
        <h3>Fiex</h3>
        <Button
          variant="secondary"
          onClick={() => openLink('https://github.com/kevinhuang001/fiex')}
        >
          GitHub
          <ExternalLink />
        </Button>
      </div>
      <div className="settings-card">
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#64748b' }}>
              <Info size={15} />
            </span>
            版本
          </div>
          <span className="settings-value">0.2.6</span>
        </div>
        <button
          className="settings-row settings-row-button"
          onClick={() => openLink('https://github.com/kevinhuang001/fiex/blob/main/THIRD-PARTY.md')}
        >
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#10b981' }}>
              <Scale size={15} />
            </span>
            开源许可
          </div>
          <ExternalLink size={14} />
        </button>
      </div>
    </>
  )
}
