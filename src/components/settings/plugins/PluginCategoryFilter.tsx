import { t } from '@/i18n'
import {
  Eye,
  FolderOpen,
  Info,
  LayoutGrid,
  MousePointerClick,
  Palette,
  Plug,
  Search,
  Terminal,
  Wrench,
  type LucideIcon,
} from 'lucide-react'
import { PLUGIN_CATEGORIES } from '@/lib/plugins'

const labels = new Map(PLUGIN_CATEGORIES.map(({ id, label }) => [id, label]))
const icons = new Map<string, LucideIcon>([
  ['info', Info],
  ['preview', Eye],
  ['contextMenus', MousePointerClick],
  ['toolbar', Wrench],
  ['search', Search],
  ['connections', Plug],
  ['hooks', FolderOpen],
  ['themes', Palette],
  ['commands', Terminal],
])

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
    <div className="plugin-category-filter" role="tablist" aria-label={t('插件分类')}>
      <button
        type="button"
        role="tab"
        aria-selected={value === 'all'}
        className={value === 'all' ? 'active' : ''}
        onClick={() => onChange('all')}
      >
        <LayoutGrid size={15} strokeWidth={1.8} />
        <span>{t('全部')}</span>
      </button>
      {visible.map(({ id }) => {
        const count = categories.find((category) => category.id === id)?.count || 0
        const Icon = icons.get(id) || Wrench
        return (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={value === id}
            className={value === id ? 'active' : ''}
            onClick={() => onChange(id)}
          >
            <Icon size={15} strokeWidth={1.8} />
            <span>{labels.get(id) || id}</span>
            <small>{count}</small>
          </button>
        )
      })}
    </div>
  )
}
