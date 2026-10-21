import type { Preferences } from '@/lib/types'
import { Eye, Home, SlidersHorizontal, Sun } from 'lucide-react'

export function AppearanceSettings({
  prefs,
  update,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
}) {
  return (
    <>
      <div className="settings-card">
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
            <select
              className="text-field settings-select"
              aria-label="启动页"
              value={prefs.startPage}
              onChange={(event) =>
                update({ startPage: event.target.value as Preferences['startPage'] })
              }
            >
              <option value="recent">最近使用</option>
              <option value="home">个人文件夹</option>
              <option value="documents">文稿</option>
              <option value="last">上次位置</option>
            </select>
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
