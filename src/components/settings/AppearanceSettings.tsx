import { useEffect, useState } from 'react'
import { installedThemes, type Theme } from '@/lib/themes'
import { SelectField } from '../ui/SelectField'
import { call, demo } from '@/lib/api'
import type { Preferences } from '@/lib/types'
import {
  Eye,
  History,
  Home,
  Image as ImageIcon,
  Palette,
  SlidersHorizontal,
  Sun,
} from 'lucide-react'

export function AppearanceSettings({
  prefs,
  update,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
}) {
  const [themes, setThemes] = useState<Theme[]>([])
  const [backgroundError, setBackgroundError] = useState('')
  const pickBackground = async () => {
    setBackgroundError('')
    try {
      const path = await call<string | null>('plugin_file_dialog', {
        mode: 'openFile',
        extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'avif'],
      })
      if (path) {
        localStorage.removeItem('fiex:background-removed')
        update({ backgroundImage: path })
      }
    } catch (error) {
      setBackgroundError(String(error))
    }
  }
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
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#8b5cf6' }}>
              <Palette size={15} />
            </span>
            主题
          </div>
          <div className="settings-row-control">
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
            <span className="settings-row-icon" style={{ background: '#ec4899' }}>
              <ImageIcon size={15} />
            </span>
            背景图片
          </div>
          <div className="settings-row-control">
            <div className="settings-inline-actions">
              <button
                type="button"
                className="settings-pill-button"
                onClick={() => void pickBackground()}
              >
                {prefs.backgroundImage ? '更换图片…' : '选择图片…'}
              </button>
              {demo && (
                <button
                  type="button"
                  className="settings-pill-button ghost"
                  onClick={() => {
                    localStorage.removeItem('fiex:background-removed')
                    update({ backgroundImage: '/demo/dog.svg', backgroundOpacity: 0.6 })
                  }}
                >
                  示例小狗
                </button>
              )}
              {prefs.backgroundImage && (
                <button
                  type="button"
                  className="settings-pill-button ghost"
                  onClick={() => {
                    localStorage.setItem('fiex:background-removed', '1')
                    update({ backgroundImage: '' })
                  }}
                >
                  移除
                </button>
              )}
            </div>
          </div>
        </div>
        {prefs.backgroundImage && (
          <div className="settings-row">
            <div className="settings-row-label">
              <span className="settings-row-icon" style={{ background: '#f59e0b' }}>
                <SlidersHorizontal size={15} />
              </span>
              背景不透明度
            </div>
            <div className="settings-row-control settings-slider">
              <input
                aria-label="背景不透明度"
                type="range"
                min="5"
                max="100"
                step="5"
                value={Math.round(prefs.backgroundOpacity * 100)}
                onChange={(event) =>
                  update({ backgroundOpacity: Number(event.target.value) / 100 })
                }
              />
              <span>{Math.round(prefs.backgroundOpacity * 100)}%</span>
            </div>
          </div>
        )}
        {backgroundError && <p className="settings-inline-error">{backgroundError}</p>}
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#64748b' }}>
              <Eye size={15} />
            </span>
            显示隐藏文件
          </div>
          <div className="settings-row-control">
            <button
              role="switch"
              aria-label="显示隐藏文件"
              aria-checked={prefs.hidden}
              className="switch"
              onClick={() => update({ hidden: !prefs.hidden })}
            >
              <i />
            </button>
          </div>
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
            <span className="settings-row-icon" style={{ background: '#14b8a6' }}>
              <History size={15} />
            </span>
            启动时恢复标签页
          </div>
          <div className="settings-row-control">
            <button
              role="switch"
              aria-checked={prefs.restoreTabs}
              aria-label="启动时恢复标签页"
              className="switch"
              onClick={() => update({ restoreTabs: !prefs.restoreTabs })}
            >
              <i />
            </button>
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
