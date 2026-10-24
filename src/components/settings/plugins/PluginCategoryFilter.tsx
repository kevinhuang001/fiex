import { PLUGIN_CATEGORIES } from '@/lib/plugins'

const labels = new Map(PLUGIN_CATEGORIES.map(({ id, label }) => [id, label]))

/** Horizontal contribution-category filter shared by installed and store lists. */
export function PluginCategoryFilter({
  categories,
  value,
  onChange,
}: {
  categories: Array<{ id: string; count: number }>
  value: string
  onChange: (value: string) => void
}) {
  const visible = PLUGIN_CATEGORIES.filter(({ id }) =>
    categories.some((category) => category.id === id),
  )
  if (!visible.length) return null
  return (
    <div className="plugin-category-filter" role="tablist" aria-label="插件分类">
      <button
        type="button"
        role="tab"
        aria-selected={value === 'all'}
        className={value === 'all' ? 'active' : ''}
        onClick={() => onChange('all')}
      >
        全部
      </button>
      {visible.map(({ id }) => {
        const count = categories.find((category) => category.id === id)?.count || 0
        return (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={value === id}
            className={value === id ? 'active' : ''}
            onClick={() => onChange(id)}
          >
            {labels.get(id) || id}
            <small>{count}</small>
          </button>
        )
      })}
    </div>
  )
}
