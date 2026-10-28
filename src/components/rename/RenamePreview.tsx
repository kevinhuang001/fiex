import { t } from '@/i18n'
import type { Entry } from '@/lib/types'

export function RenamePreview({
  entries,
  names,
  validation,
}: {
  entries: Entry[]
  names: string[]
  validation: Map<string, string>
}) {
  return (
    <div className="batch-rename-list">
      {(entries || []).map((entry, index) => {
        const name = names[index] || t('（空）')
        const problem = validation.get(entry.path)
        const changedRow = name !== entry.name
        return (
          <div
            className={`batch-rename-row ${problem ? 'invalid' : ''} ${changedRow ? 'changed' : ''}`}
            key={entry.path}
          >
            <span title={entry.name}>{entry.name}</span>
            <b>→</b>
            <span title={name}>{name}</span>
            {problem && <em>{problem}</em>}
          </div>
        )
      })}
    </div>
  )
}
