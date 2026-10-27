import { t } from '@/i18n'
import { useEffect, useState } from 'react'
import { installedThemes, type Theme } from '@/lib/themes'
import { SelectField } from '../ui/SelectField'
import { call, demo } from '@/lib/api'
import type { Preferences } from '@/lib/types'
import { Image as ImageIcon, Languages, Palette, SlidersHorizontal, Sun } from 'lucide-react'

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
            <span className="settings-row-icon" style={{ background: '#14b8a6' }}>
              <Languages size={15} />
            </span>
            {t('语言')}
          </div>
          <div className="settings-row-control">
            <SelectField
              label={t('语言')}
              value={prefs.locale}
              options={[
                { value: 'zh-CN', label: t('简体中文') },
                { value: 'zh-TW', label: t('繁體中文') },
                { value: 'en', label: 'English' },
              ]}
              onChange={(locale) => update({ locale: locale as Preferences['locale'] })}
            />
          </div>
        </div>
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#8b5cf6' }}>
              <Palette size={15} />
            </span>
            {t('主题')}
          </div>
          <div className="settings-row-control">
            <SelectField
              label={t('主题')}
              value={prefs.themeId || ''}
              options={[
                { value: '', label: t('Fiex 默认') },
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
            {t('外观')}
          </div>
          <div className="settings-row-control">
            <div className="segmented">
              {(['system', 'light', 'dark'] as const).map((theme, index) => (
                <button
                  key={theme}
                  aria-pressed={prefs.theme === theme}
                  onClick={() => update({ theme })}
                >
                  {[t('跟随系统'), t('浅色'), t('深色')][index]}
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
            {t('背景图片')}
          </div>
          <div className="settings-row-control">
            <div className="settings-inline-actions">
              <button
                type="button"
                className="settings-pill-button"
                onClick={() => void pickBackground()}
              >
                {prefs.backgroundImage ? t('更换图片…') : t('选择图片…')}
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
                  {t('示例小狗')}
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
                  {t('移除')}
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
              {t('背景不透明度')}
            </div>
            <div className="settings-row-control settings-slider">
              <input
                aria-label={t('背景不透明度')}
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
            <span className="settings-row-icon" style={{ background: '#6366f1' }}>
              <SlidersHorizontal size={15} />
            </span>
            {t('字体大小')}
          </div>
          <div className="settings-row-control settings-slider">
            <input
              aria-label={t('字体大小')}
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
