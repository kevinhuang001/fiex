import { useEffect, useState } from 'react'
import { installedThemes, type Theme } from '@/lib/themes'
import { SelectField } from '../ui/SelectField'
import type { Preferences } from '@/lib/types'
import { Eye, Home, SlidersHorizontal, Sun } from 'lucide-react'

export function AppearanceSettings({
  prefs,
  update,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
}) {
  const [themes, setThemes] = useState<Theme[]>([])
  useEffect(() => {
    let alive = true
    const refresh = () => {
      void installedThemes()
        .then((items) => {
          if (alive) setThemes(items)
        })
        .catch(() => {})
    }
    refresh()
    window.addEventListener('fiex:plugins-changed', refresh)
    return () => {
      alive = false
      window.removeEventListener('fiex:plugins-changed', refresh)
    }
  }, [])
  return (
    <>
      <div className="settings-card">
        <div className="settings-row">
          <div className="settings-row-label">主题</div>
          <SelectField
            label="主题"
            value={prefs.themeId || ''}
            options={[
              { value: '', label: 'Fiex 默认' },
              ...themes.map((theme) => ({ value: theme.id, label: theme.name })),
            ]}
            onChange={(themeId) => update({ themeId })}
          />
        </div>
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#3b82f6' }}>
              <Sun size={15} />
            </span>
            外观
          </div>
          <div className="settings-row-control">
            <div className="segmented">
              {(['system', 'light', 'dark'] as const).map((theme, index) => (
                <button
                  key={theme}
                  aria-pressed={prefs.theme === theme}
                  onClick={() => update({ theme })}
                >
                  {['跟随系统', '浅色', '深色'][index]}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#64748b' }}>
              <Eye size={15} />
            </span>
            显示隐藏文件
          </div>
          <button
            role="switch"
            aria-checked={prefs.hidden}
            className="switch"
            onClick={() => update({ hidden: !prefs.hidden })}
          >
            <i />
          </button>
        </div>
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#0ea5e9' }}>
              <Home size={15} />
            </span>
            启动页
          </div>
          <div className="settings-row-control">
            <SelectField
              label="启动页"
              value={prefs.startPage}
              options={[
                { value: 'recent', label: '最近使用' },
                { value: 'home', label: '个人文件夹' },
                { value: 'documents', label: '文稿' },
                { value: 'last', label: '上次位置' },
              ]}
              onChange={(startPage) => update({ startPage: startPage as Preferences['startPage'] })}
            />
          </div>
        </div>
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#6366f1' }}>
              <SlidersHorizontal size={15} />
            </span>
            字体大小
          </div>
          <div className="settings-row-control settings-slider">
            <input
              aria-label="字体大小"
              type="range"
              min="8"
              max="18"
              step="1"
              value={prefs.fontSize}
              onChange={(event) => update({ fontSize: +event.target.value })}
            />
            <span>{prefs.fontSize}px</span>
          </div>
        </div>
      </div>
    </>
  )
}
