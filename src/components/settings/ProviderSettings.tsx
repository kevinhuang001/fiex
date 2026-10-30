import { t, getLocale } from '@/i18n'
import { call } from '@/lib/api'
import { ChevronDown, ChevronUp, LoaderCircle, RotateCcw } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'

interface ProviderEntry {
  id: string
  name: string
  defaultPriority: number
  builtin: boolean
  locked?: boolean
  available?: boolean
  modes?: string[]
}

interface Preference {
  order: string[]
  disabled: string[]
}

type Preferences = Record<string, Preference>
type Catalog = Record<string, ProviderEntry[]>

const FEATURES = [
  ['search', '搜索'],
  ['preview', '预览'],
  ['info', '文件信息'],
  ['decorations', '装饰徽标'],
  ['contextMenus', '右键菜单'],
  ['hooks', 'Hooks'],
] as const

/** One arrange/save model for every priority-aware feature: the stored order
 *  wins, new providers keep their manifest priority, locked Core providers
 *  stay pinned at the end and can never be disabled. */
function arrange(entries: ProviderEntry[], preference?: Preference) {
  const rank = new Map((preference?.order ?? []).map((id, index) => [id, index]))
  return [...entries].sort((left, right) => {
    if (!!left.locked !== !!right.locked) return left.locked ? 1 : -1
    const leftRank = rank.get(left.id) ?? Number.MAX_SAFE_INTEGER
    const rightRank = rank.get(right.id) ?? Number.MAX_SAFE_INTEGER
    if (leftRank !== rightRank) return leftRank - rightRank
    return right.defaultPriority - left.defaultPriority
  })
}

export function ProviderSettings() {
  const [catalog, setCatalog] = useState<Catalog>({})
  const [preferences, setPreferences] = useState<Preferences>({})
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    let active = true
    void Promise.all([
      call<{ features: Catalog }>('provider_catalog'),
      call<Preferences>('provider_preferences'),
    ])
      .then(([nextCatalog, nextPreferences]) => {
        if (!active) return
        setCatalog(nextCatalog?.features ?? {})
        setPreferences(nextPreferences ?? {})
      })
      .catch((reason) => active && setError(String(reason)))
      .finally(() => active && setLoading(false))
    return () => {
      active = false
    }
  }, [])

  const rows = useMemo(
    () =>
      Object.fromEntries(
        FEATURES.map(([feature]) => [
          feature,
          arrange(catalog[feature] ?? [], preferences[feature]),
        ]),
      ) as Record<string, ProviderEntry[]>,
    [catalog, preferences, getLocale()],
  )

  const save = async (feature: string, order: string[], disabled: string[]) => {
    setSaving(feature)
    setError('')
    try {
      const next = await call<Preferences>('set_provider_preference', {
        feature,
        order,
        disabled,
      })
      setPreferences(next ?? {})
    } catch (reason) {
      setError(String(reason))
    } finally {
      setSaving('')
    }
  }

  const currentOrder = (feature: string) =>
    rows[feature].filter((entry) => !entry.locked).map((entry) => entry.id)

  const move = (feature: string, id: string, delta: number) => {
    const list = [...currentOrder(feature)]
    const from = list.indexOf(id)
    const to = from + delta
    if (from < 0 || to < 0 || to >= list.length) return
    list.splice(to, 0, list.splice(from, 1)[0])
    void save(feature, list, preferences[feature]?.disabled ?? [])
  }

  const toggle = (feature: string, id: string) => {
    const disabled = new Set(preferences[feature]?.disabled ?? [])
    if (disabled.has(id)) disabled.delete(id)
    else disabled.add(id)
    void save(feature, currentOrder(feature), [...disabled])
  }

  const reset = async (feature: string) => {
    setSaving(feature)
    setError('')
    try {
      setPreferences((await call<Preferences>('reset_provider_preference', { feature })) ?? {})
    } catch (reason) {
      setError(String(reason))
    } finally {
      setSaving('')
    }
  }

  if (loading) {
    return (
      <div className="settings-card">
        <LoaderCircle className="spin" size={16} />
      </div>
    )
  }

  return (
    <>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>{t('提供程序优先级')}</span>
        </div>
        <p className="settings-hint">
          {t(
            '按功能调整提供程序的执行顺序：用户顺序优先于插件声明的默认优先级，内置保底项不可移动或关闭。',
          )}
        </p>
        {error && <p className="settings-error">{error}</p>}
      </div>
      {FEATURES.map(([feature, label]) => {
        const list = rows[feature] ?? []
        const disabled = new Set(preferences[feature]?.disabled ?? [])
        return (
          <div className="settings-card" key={feature}>
            <div className="settings-card-header">
              <span>
                {t(label)}
                <small className="ignore-count">{list.length}</small>
              </span>
              <button
                className="settings-reset"
                disabled={saving === feature}
                onClick={() => void reset(feature)}
              >
                <RotateCcw size={11} />
                {t('恢复默认')}
              </button>
            </div>
            {list.length === 0 && <p className="settings-empty">{t('尚未安装提供此功能的插件')}</p>}
            {list.map((entry, index) => {
              const movable = !entry.locked
              const enabled = !disabled.has(entry.id)
              return (
                <div className={`provider-row ${enabled ? '' : 'disabled'}`} key={entry.id}>
                  <div className="provider-main">
                    <span className="provider-name">
                      {entry.name}
                      {entry.builtin && <em className="provider-badge">{t('内置')}</em>}
                      {entry.available === false && (
                        <em className="provider-badge warn">{t('不可用')}</em>
                      )}
                    </span>
                    <code className="provider-id">{entry.id}</code>
                    {entry.modes && entry.modes.length > 0 && (
                      <small className="provider-modes">
                        {entry.modes
                          .map((mode) => (mode === 'content' ? t('文件内容') : t('文件名')))
                          .join(' · ')}
                      </small>
                    )}
                  </div>
                  <span className="provider-priority" title={t('默认优先级')}>
                    {entry.defaultPriority}
                  </span>
                  <button
                    className="icon-button"
                    aria-label={t('上移')}
                    disabled={!movable || index === 0}
                    onClick={() => move(feature, entry.id, -1)}
                  >
                    <ChevronUp size={14} />
                  </button>
                  <button
                    className="icon-button"
                    aria-label={t('下移')}
                    disabled={!movable || index === list.length - 1}
                    onClick={() => move(feature, entry.id, 1)}
                  >
                    <ChevronDown size={14} />
                  </button>
                  <label
                    className="provider-toggle"
                    title={entry.locked ? t('内置保底项不可关闭') : ''}
                  >
                    <input
                      type="checkbox"
                      checked={enabled}
                      disabled={!!entry.locked}
                      onChange={() => toggle(feature, entry.id)}
                    />
                    {enabled ? t('启用') : t('已停用')}
                  </label>
                </div>
              )
            })}
          </div>
        )
      })}
    </>
  )
}
