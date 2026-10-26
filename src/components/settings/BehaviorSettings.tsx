import { t } from '@/i18n'
import { Eye, History, Home } from 'lucide-react'
import { SelectField } from '../ui/SelectField'
import type { Preferences } from '@/lib/types'

export function BehaviorSettings({
  prefs,
  update,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
}) {
  return (
    <div className="settings-card">
      <div className="settings-row">
        <div className="settings-row-label">
          <span className="settings-row-icon" style={{ background: '#64748b' }}>
            <Eye size={15} />
          </span>
          {t('显示隐藏文件')}
        </div>
        <div className="settings-row-control">
          <button
            role="switch"
            aria-label={t('显示隐藏文件')}
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
          {t('启动页')}
        </div>
        <div className="settings-row-control">
          <SelectField
            label={t('启动页')}
            value={prefs.startPage}
            options={[
              { value: 'recent', label: t('最近使用') },
              { value: 'home', label: t('个人文件夹') },
              { value: 'documents', label: t('文稿') },
              { value: 'last', label: t('上次位置') },
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
          {t('启动时恢复标签页')}
        </div>
        <div className="settings-row-control">
          <button
            role="switch"
            aria-checked={prefs.restoreTabs}
            aria-label={t('启动时恢复标签页')}
            className="switch"
            onClick={() => update({ restoreTabs: !prefs.restoreTabs })}
          >
            <i />
          </button>
        </div>
      </div>
    </div>
  )
}
