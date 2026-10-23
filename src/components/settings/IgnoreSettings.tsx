import { defaultDisplayIgnore } from '@/lib/ignores'
import type { Preferences } from '@/lib/types'
import { Plus, RotateCcw, X } from 'lucide-react'
import { useState } from 'react'
function IgnoreEditor({
  title,
  patterns,
  onChange,
  onReset,
}: {
  title: string
  patterns: string[]
  onChange: (patterns: string[]) => void
  onReset: () => void
}) {
  const [draft, setDraft] = useState('')
  const add = () => {
    const pattern = draft.trim()
    if (!pattern) return
    if (!patterns.includes(pattern)) onChange([...patterns, pattern])
    setDraft('')
  }
  return (
    <div className="settings-card ignore-card">
      <div className="settings-card-header">
        <span>
          {title}
          <small className="ignore-count">{patterns.length}</small>
        </span>
        <button className="settings-reset" onClick={onReset}>
          <RotateCcw size={11} />
          恢复预设
        </button>
      </div>
      <div className="ignore-chips">
        {patterns.map((pattern) => (
          <span className="ignore-chip" key={pattern}>
            <code>{pattern}</code>
            <button
              aria-label={`删除 ${pattern}`}
              onClick={() => onChange(patterns.filter((item) => item !== pattern))}
            >
              <X size={10} />
            </button>
          </span>
        ))}
      </div>
      <div className="ignore-add">
        <input
          value={draft}
          placeholder="添加规则，例如 node_modules 或 build/*"
          aria-label={`添加${title}规则`}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              add()
            }
          }}
        />
        <button aria-label={`添加${title}规则`} disabled={!draft.trim()} onClick={add}>
          <Plus size={14} />
        </button>
      </div>
    </div>
  )
}

export function IgnoreSettings({
  prefs,
  update,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
}) {
  return (
    <>
      {(
        [
          {
            key: 'displayIgnore',
            title: '显示忽略',
            reset: defaultDisplayIgnore,
          },
        ] as const
      ).map(({ key, title, reset }) => (
        <IgnoreEditor
          key={key}
          title={title}
          patterns={prefs[key]}
          onChange={(patterns) => update({ [key]: patterns } as Partial<Preferences>)}
          onReset={() => update({ [key]: reset() } as Partial<Preferences>)}
        />
      ))}
    </>
  )
}
