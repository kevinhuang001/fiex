import { t, getLocale } from '@/i18n'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import {
  Search,
  Eye,
  FileText,
  Sparkles,
  MousePointer2,
  Workflow,
  LoaderCircle,
  RotateCcw,
} from 'lucide-react'
import { ProviderRow } from './ProviderRow'
import { useEffect, useMemo, useState } from 'react'

export interface ProviderEntry {
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
  const [feature, setFeature] = useState('search')
  const [dragged, setDragged] = useState<string | null>(null)
  const [catalog, setCatalog] = useState<Catalog>({})
  const [preferences, setPreferences] = useState<Preferences>({})
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState('')

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
      .catch((reason) => {
        if (active) reportError(reason, 'plugins:providers')
      })
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
    try {
      const next = await call<Preferences>('set_provider_preference', {
        feature,
        order,
        disabled,
      })
      setPreferences(next ?? {})
    } catch (reason) {
      reportError(reason, 'plugins:providers')
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
    try {
      setPreferences((await call<Preferences>('reset_provider_preference', { feature })) ?? {})
    } catch (reason) {
      reportError(reason, 'plugins:providers')
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

  const list = rows[feature] ?? []
  const disabled = new Set(preferences[feature]?.disabled ?? [])
  const icons = [Search, Eye, FileText, Sparkles, MousePointer2, Workflow]
  const activeIndex = Math.max(
    0,
    FEATURES.findIndex(([id]) => id === feature),
  )
  const ActiveIcon = icons[activeIndex]
  const enabledCount = list.filter((entry) => entry.locked || !disabled.has(entry.id)).length
  const drop = (target: string) => {
    const order = currentOrder(feature)
    if (!dragged || dragged === target || !order.includes(target)) return
    const targetIndex = order.indexOf(target)
    order.splice(order.indexOf(dragged), 1)
    order.splice(targetIndex, 0, dragged)
    setDragged(null)
    void save(feature, order, [...disabled])
  }
  return (
    <section className="provider-manager">
      <div className="provider-intro">
        <div className="provider-intro-icon">
          <Workflow size={23} />
        </div>
        <div>
          <h2>{t('提供程序')}</h2>
          <p>{t('选择偏好的工具，按你的习惯安排顺序。')}</p>
        </div>
      </div>
      <div className="provider-feature-tabs" role="tablist" aria-label={t('功能分类')}>
        {FEATURES.map(([id, label], index) => {
          const Icon = icons[index]
          return (
            <button
              key={id}
              role="tab"
              id={`provider-tab-${id}`}
              aria-selected={feature === id}
              aria-controls={`provider-panel-${id}`}
              onClick={() => {
                setFeature(id)
                setDragged(null)
              }}
            >
              <Icon size={16} />
              <span>{t(label)}</span>
              <small>{rows[id]?.length ?? 0}</small>
            </button>
          )
        })}
      </div>
      <div
        className="provider-stack"
        role="tabpanel"
        id={`provider-panel-${feature}`}
        aria-labelledby={`provider-tab-${feature}`}
      >
        <div className="provider-stack-heading">
          <div className="provider-stack-title">
            <span className="provider-stack-icon">
              <ActiveIcon size={17} />
            </span>
            <div>
              <h3>{t('执行顺序')}</h3>
              <p>{t('拖动调整顺序，也可以使用上下移动按钮。')}</p>
            </div>
          </div>
          <div className="provider-stack-actions">
            <span className="provider-stack-count" title={t('已启用')}>
              {enabledCount}/{list.length}
            </span>
            <button
              className="settings-reset"
              disabled={Boolean(saving)}
              onClick={() => void reset(feature)}
            >
              <RotateCcw size={13} />
              {t('恢复默认')}
            </button>
          </div>
        </div>
        {list.length === 0 && (
          <div className="provider-empty">
            <Workflow size={28} />
            <p>{t('尚未安装提供此功能的插件')}</p>
          </div>
        )}
        <div className="provider-stack-list">
          {list.map((entry, index) => (
            <ProviderRow
              key={entry.id}
              entry={entry}
              index={index}
              last={index === currentOrder(feature).length - 1}
              enabled={!disabled.has(entry.id)}
              busy={Boolean(saving)}
              dragging={dragged === entry.id}
              onDrag={() => setDragged(entry.id)}
              onDragEnd={() => setDragged(null)}
              onDrop={() => drop(entry.id)}
              onMove={(delta) => move(feature, entry.id, delta)}
              onToggle={() => toggle(feature, entry.id)}
            />
          ))}
        </div>
        <div className="provider-stack-note">
          <Sparkles size={14} />
          <span>{t('内置保底提供程序始终保留，让功能随时可用。')}</span>
          {saving && <LoaderCircle className="spin" size={14} />}
        </div>
      </div>
    </section>
  )
}
