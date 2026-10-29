import { useEffect, useState } from 'react'
import { call } from '@/lib/api'
import { t } from '@/i18n'
import type { PluginInfo } from '@/lib/plugins'

type Preference = { order: string[]; disabled: string[] }
type Preferences = Record<string, Preference>
const features = [
  ['search', '搜索'],
  ['preview', '预览'],
  ['info', '简介'],
  ['decorations', '装饰'],
  ['contextMenus', '右键菜单'],
  ['hooks', '钩子'],
  ['connectionContextMenus', '连接右键菜单'],
] as const
const empty = (): Preference => ({ order: [], disabled: [] })

export function ProviderPrioritySettings({ plugins }: { plugins: PluginInfo[] }) {
  const [prefs, setPrefs] = useState<Preferences>({})
  const [error, setError] = useState('')
  useEffect(() => {
    void call<Preferences>('provider_preferences')
      .then(setPrefs)
      .catch((cause) => setError(String(cause)))
  }, [])
  const save = (feature: string, preference: Preference) => {
    setPrefs((current) => ({ ...current, [feature]: preference }))
    void call('provider_preferences_set', { feature, preference }).catch((cause) =>
      setError(String(cause)),
    )
  }
  return (
    <section className="provider-priority-settings">
      <h3>{t('Provider 优先级')}</h3>
      <p>{t('拖动顺序或使用上移、下移按钮；用户顺序优先于插件默认优先级。')}</p>
      {error && <p role="alert">{error}</p>}
      {features.map(([feature, label]) => {
        const available = plugins.flatMap((plugin) => {
          const contribution = (plugin.extensions as Record<string, unknown> | undefined)?.[feature]
          if (!contribution) return []
          const declared =
            typeof contribution === 'object' && contribution !== null
              ? (contribution as { priority?: number })
              : {}
          return [
            {
              id: `plugin:${plugin.id}`,
              name: plugin.name,
              priority: declared.priority ?? 50,
              available: plugin.enabled && plugin.approved && plugin.backendAvailable,
            },
          ]
        })
        if (feature === 'search')
          available.push({ id: 'core.search', name: 'Fiex', priority: -1000, available: true })
        if (feature === 'preview')
          available.push({ id: 'builtin:fallback', name: 'Fiex', priority: -1000, available: true })
        const pref = prefs[feature] ?? empty()
        const rank = (id: string) => pref.order.indexOf(id)
        available.sort((a, b) => {
          const left = rank(a.id)
          const right = rank(b.id)
          if (left >= 0 && right >= 0) return left - right
          if (left >= 0) return -1
          if (right >= 0) return 1
          return b.priority - a.priority || a.id.localeCompare(b.id)
        })
        if (feature === 'search')
          available.sort((a, b) => (a.id === 'core.search' ? 1 : b.id === 'core.search' ? -1 : 0))
        return (
          <div key={feature}>
            <h4>{t(label)}</h4>
            <button type="button" onClick={() => save(feature, empty())}>
              {t('恢复默认')}
            </button>
            <ol>
              {available.map((provider, index) => {
                const locked = !provider.id.startsWith('plugin:')
                const move = (to: number) => {
                  if (
                    to < 0 ||
                    to >= available.length ||
                    locked ||
                    !available[to].id.startsWith('plugin:')
                  )
                    return
                  const order = available.map((entry) => entry.id)
                  order.splice(index, 1)
                  order.splice(to, 0, provider.id)
                  save(feature, { ...pref, order })
                }
                return (
                  <li
                    key={provider.id}
                    draggable={!locked}
                    onDragStart={(event) => event.dataTransfer.setData('text/plain', provider.id)}
                    onDragOver={(event) => {
                      if (!locked) event.preventDefault()
                    }}
                    onDrop={(event) => {
                      event.preventDefault()
                      const from = available.findIndex(
                        (entry) => entry.id === event.dataTransfer.getData('text/plain'),
                      )
                      if (from >= 0) {
                        const order = available.map((entry) => entry.id)
                        order.splice(from, 1)
                        order.splice(index, 0, available[from].id)
                        save(feature, { ...pref, order })
                      }
                    }}
                  >
                    <span>
                      {provider.name} · {provider.id} · {t('默认优先级')} {provider.priority}
                    </span>
                    {!provider.available && <span>{t('不可用或未授权')}</span>}
                    <button
                      type="button"
                      disabled={locked || index === 0}
                      onClick={() => move(index - 1)}
                    >
                      {t('上移')}
                    </button>
                    <button
                      type="button"
                      disabled={locked || index === available.length - 1}
                      onClick={() => move(index + 1)}
                    >
                      {t('下移')}
                    </button>
                    <label>
                      <input
                        type="checkbox"
                        checked={!pref.disabled.includes(provider.id)}
                        disabled={locked}
                        onChange={(event) =>
                          save(feature, {
                            ...pref,
                            disabled: event.target.checked
                              ? pref.disabled.filter((id) => id !== provider.id)
                              : [...pref.disabled, provider.id],
                          })
                        }
                      />
                      {t('启用')}
                    </label>
                  </li>
                )
              })}
            </ol>
          </div>
        )
      })}
    </section>
  )
}
