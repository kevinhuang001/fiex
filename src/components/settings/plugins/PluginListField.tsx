import { useEffect, useRef, useState } from 'react'
import { Modal } from '@/components/ui/Modal'
import { Button } from '@/components/ui/button'
import type { PluginField } from '@/lib/plugins'

export function PluginListField({
  field,
  values,
  onChange,
}: {
  field: PluginField
  values: string[]
  onChange: (values: string[]) => void
}) {
  const [open, setOpen] = useState(false)
  const [draft, setDraft] = useState<string[]>(values)
  const [selected, setSelected] = useState<number | null>(null)
  const [edit, setEdit] = useState<{ index: number | null; value: string } | null>(null)
  const editInput = useRef<HTMLInputElement>(null)

  const show = () => {
    setDraft(values)
    setSelected(values.length ? 0 : null)
    setEdit(null)
    setOpen(true)
  }
  const commit = () => {
    onChange(draft.map((value) => value.trim()).filter(Boolean))
    setOpen(false)
  }
  const saveEdit = () => {
    if (!edit) return
    const value = edit.value.trim()
    setDraft((old) => {
      if (edit.index === null) {
        if (!value) return old
        setSelected(old.length)
        return [...old, value]
      }
      if (!value) return old.filter((_, index) => index !== edit.index)
      return old.map((item, index) => (index === edit.index ? value : item))
    })
    setEdit(null)
  }

  useEffect(() => {
    if (!edit) return
    const timer = window.setTimeout(() => editInput.current?.focus(), 40)
    return () => window.clearTimeout(timer)
  }, [edit])

  return (
    <>
      <button type="button" className="plugin-list-trigger" onClick={show}>
        <span>编辑条目</span>
        <small>{values.length} 项</small>
      </button>
      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={field.label}
        className="plugin-list-modal"
      >
        <div className="plugin-list-editor">
          <div className="plugin-list-editor-rows" role="listbox" aria-label={field.label}>
            {draft.map((value, index) => (
              <button
                key={index}
                type="button"
                role="option"
                aria-selected={selected === index}
                className={`plugin-list-entry ${selected === index ? 'selected' : ''}`}
                onClick={() => setSelected(index)}
                onDoubleClick={() => setEdit({ index, value })}
              >
                {value}
              </button>
            ))}
            {!draft.length && <p className="plugin-list-empty">还没有条目。</p>}
          </div>
          <div className="plugin-list-editor-actions">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setEdit({ index: null, value: '' })}
            >
              新建
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={selected === null}
              onClick={() =>
                selected !== null && setEdit({ index: selected, value: draft[selected] })
              }
            >
              编辑
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={selected === null}
              onClick={() => {
                if (selected === null) return
                setDraft((old) => old.filter((_, index) => index !== selected))
                setSelected(null)
              }}
            >
              删除
            </Button>
          </div>
        </div>
        <footer className="plugin-list-footer">
          <Button variant="outline" onClick={() => setOpen(false)}>
            取消
          </Button>
          <Button onClick={commit}>确定</Button>
        </footer>
      </Modal>
      <Modal
        open={Boolean(edit)}
        onClose={() => setEdit(null)}
        title={edit?.index === null ? '新建条目' : '编辑条目'}
        className="plugin-list-confirm"
      >
        {edit && (
          <>
            <label className="plugin-settings-field plugin-list-confirm-field">
              <span className="plugin-settings-label">{field.label}</span>
              <input
                ref={editInput}
                value={edit.value}
                placeholder={field.placeholder}
                onChange={(event) => setEdit({ ...edit, value: event.target.value })}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault()
                    saveEdit()
                  }
                }}
              />
            </label>
            <footer className="plugin-list-footer">
              <Button variant="outline" onClick={() => setEdit(null)}>
                取消
              </Button>
              <Button onClick={saveEdit}>{edit.index === null ? '添加' : '确定'}</Button>
            </footer>
          </>
        )}
      </Modal>
    </>
  )
}
