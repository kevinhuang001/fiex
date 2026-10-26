import { t, getLocale } from '@/i18n'
import { useEffect, useMemo, useState } from 'react'
import { AlertCircle, Hash, CaseSensitive, Replace, RotateCcw, Wand2 } from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import type { Entry } from '@/lib/types'
import {
  buildRenames,
  DEFAULT_RENAME_OPTIONS,
  RENAME_PRESETS,
  RENAME_TOKENS,
  validateRenames,
  type RenameCase,
  type RenameExtension,
  type RenameOptions,
} from '@/lib/rename'

type Rename = { path: string; name: string }

export function BatchRenameDialog({
  entries,
  onClose,
  onApply,
}: {
  entries: Entry[] | null
  onClose: () => void
  onApply: (renames: Rename[]) => Promise<void>
}) {
  const [options, setOptions] = useState<RenameOptions>(DEFAULT_RENAME_OPTIONS)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!entries) return
    setOptions(DEFAULT_RENAME_OPTIONS)
    setBusy(false)
    setError('')
  }, [entries])

  const update = (patch: Partial<RenameOptions>) => setOptions((value) => ({ ...value, ...patch }))
  const built = useMemo(() => {
    if (!entries) return { names: [] as string[], error: '' }
    try {
      return { names: buildRenames(entries, options), error: '' }
    } catch (cause) {
      return { names: [] as string[], error: t('正则表达式无效：{0}', String(cause)) }
    }
  }, [entries, options, getLocale()])
  const validation = useMemo(
    () => (entries ? validateRenames(entries, built.names) : new Map<string, string>()),
    [entries, built.names, getLocale()],
  )
  const changed = (entries || [])
    .map((entry, index) => ({ path: entry.path, name: built.names[index] ?? '' }))
    .filter((item, index) => item.name !== entries?.[index]?.name)
  const canApply = !busy && !built.error && changed.length > 0 && validation.size === 0

  async function submit() {
    if (!canApply) return
    setBusy(true)
    setError('')
    try {
      await onApply(changed)
    } catch (cause) {
      setError(String(cause))
      setBusy(false)
    }
  }

  return (
    <Modal
      open={Boolean(entries)}
      onClose={onClose}
      title={t('批量重命名{0}', entries ? ` · ${entries.length} 项` : '')}
      wide
      className="batch-rename-modal"
    >
      <div className="batch-rename">
        <div className="batch-rename-side">
          <section className="batch-rename-section">
            <header>
              <Wand2 size={14} /> {t('文件名格式')}
            </header>
            <input
              className="text-field batch-rename-template-input"
              value={options.template}
              spellCheck={false}
              aria-label={t('文件名格式')}
              onChange={(event) => update({ template: event.target.value })}
            />
            <div className="batch-rename-presets">
              {RENAME_PRESETS.map((preset) => (
                <button
                  type="button"
                  key={preset.label}
                  onClick={() => update({ template: preset.template })}
                >
                  {preset.label}
                </button>
              ))}
            </div>
            <div className="batch-rename-tokens">
              {RENAME_TOKENS.map(({ token, label }) => (
                <button
                  type="button"
                  key={token}
                  title={label}
                  onClick={() => update({ template: options.template + token })}
                >
                  {token}
                </button>
              ))}
            </div>
          </section>

          <section className="batch-rename-section">
            <header>
              <Replace size={14} /> {t('查找与替换')}
            </header>
            <div className="batch-rename-pair">
              <input
                className="text-field"
                value={options.find}
                placeholder={t('查找')}
                aria-label={t('查找')}
                onChange={(event) => update({ find: event.target.value })}
              />
              <input
                className="text-field"
                value={options.replace}
                placeholder={t('替换为')}
                aria-label={t('替换为')}
                onChange={(event) => update({ replace: event.target.value })}
              />
            </div>
            <div className="batch-rename-checks">
              <label>
                <input
                  type="checkbox"
                  checked={options.regex}
                  onChange={(event) => update({ regex: event.target.checked })}
                />
                {t('正则表达式')}
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={options.applyToExtension}
                  onChange={(event) => update({ applyToExtension: event.target.checked })}
                />
                {t('包含扩展名')}
              </label>
            </div>
            {built.error && (
              <p className="batch-rename-inline-error">
                <AlertCircle size={13} /> {built.error}
              </p>
            )}
          </section>

          <section className="batch-rename-section">
            <header>
              <CaseSensitive size={14} /> {t('大小写与扩展名')}
            </header>
            <div className="batch-rename-segmented" role="group" aria-label={t('大小写')}>
              {(
                [
                  ['keep', t('保持')],
                  ['lower', t('小写')],
                  ['upper', t('大写')],
                  ['title', t('首字母')],
                ] as Array<[RenameCase, string]>
              ).map(([value, label]) => (
                <button
                  type="button"
                  key={value}
                  aria-pressed={options.caseMode === value}
                  onClick={() => update({ caseMode: value })}
                >
                  {label}
                </button>
              ))}
            </div>
            <label className="batch-rename-select">
              <span>{t('扩展名')}</span>
              <select
                value={options.extensionMode}
                aria-label={t('扩展名处理')}
                onChange={(event) =>
                  update({ extensionMode: event.target.value as RenameExtension })
                }
              >
                <option value="keep">{t('保留原样')}</option>
                <option value="lower">{t('转为小写')}</option>
                <option value="upper">{t('转为大写')}</option>
                <option value="remove">{t('移除')}</option>
              </select>
            </label>
          </section>

          <section className="batch-rename-section">
            <header>
              <Hash size={14} /> {t('序号')}
            </header>
            <div className="batch-rename-numbers">
              <label>
                <span>{t('起始')}</span>
                <input
                  type="number"
                  min={0}
                  value={options.start}
                  onChange={(event) => update({ start: Number(event.target.value) || 0 })}
                />
              </label>
              <label>
                <span>{t('步长')}</span>
                <input
                  type="number"
                  min={1}
                  value={options.step}
                  onChange={(event) =>
                    update({ step: Math.max(1, Number(event.target.value) || 1) })
                  }
                />
              </label>
              <label>
                <span>{t('位数')}</span>
                <input
                  type="number"
                  min={1}
                  max={12}
                  value={options.padding}
                  onChange={(event) =>
                    update({ padding: Math.min(12, Math.max(1, Number(event.target.value) || 1)) })
                  }
                />
              </label>
            </div>
            <p className="batch-rename-hint">
              {t('模板里用')}
              {'{n}'} {t('使用位数；也可以用')}
              {'{n2}'} / {'{n3}'} {t('强制宽度。')}
            </p>
          </section>
        </div>

        <div className="batch-rename-preview">
          <header>
            <span>
              {t('预览 ·')}
              {entries?.length || 0} {t('项')}
              {changed.length > 0 && t(' · {0} 个改动', changed.length)}
            </span>
            <button
              type="button"
              onClick={() => setOptions(DEFAULT_RENAME_OPTIONS)}
              title={t('重置规则')}
            >
              <RotateCcw size={13} /> {t('重置')}
            </button>
          </header>
          <div className="batch-rename-list">
            {(entries || []).map((entry, index) => {
              const name = built.names[index] || t('（空）')
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
          {error && (
            <p className="batch-rename-error" role="alert">
              {error}
            </p>
          )}
          <footer className="batch-rename-footer">
            <span>
              {validation.size > 0
                ? t('{0} 个名称需要处理', validation.size)
                : changed.length > 0
                  ? t('将重命名 {0} 项', changed.length)
                  : t('还没有改动')}
            </span>
            <Button variant="outline" onClick={onClose} disabled={busy}>
              {t('取消')}
            </Button>
            <Button onClick={() => void submit()} disabled={!canApply}>
              {busy ? t('正在重命名…') : t('开始重命名')}
            </Button>
          </footer>
        </div>
      </div>
    </Modal>
  )
}
