import { t } from '@/i18n'
import { tagHex } from '@/lib/tags'
import { Fragment } from 'react'
import type { SidebarProps } from './types'

/** The tag filter list. */
export function Tags({ x, prefs }: Pick<SidebarProps, 'x' | 'prefs'>) {
  const allTags = prefs.tags
  return (
    <Fragment>
      <div className="sidebar-section-title">
        <span className="sidebar-label">{t('标签')}</span>
      </div>
      {allTags.map((tag) => (
        <button
          key={tag.name}
          className={`nav-item tag-nav ${x.tagFilter === tag.name ? 'active' : ''}`}
          onClick={() => x.setTagFilter(x.tagFilter === tag.name ? null : tag.name)}
        >
          <span className="tag-dot" style={{ background: tagHex(tag) }} />
          <span>{tag.name}</span>
        </button>
      ))}
    </Fragment>
  )
}
