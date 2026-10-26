import { t } from '@/i18n'
import { CreateTagDialog } from '@/components/CreateTagDialog'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import { tagHex } from '@/lib/tags'
import type { Preferences, Tag } from '@/lib/types'
import { Pencil, Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'

export function TagSettings({
  prefs,
  update,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
}) {
  const [editing, setEditing] = useState<Tag | 'new' | null>(null)
  const [deleting, setDeleting] = useState<Tag | null>(null)
  const tags = prefs.tags
  return (
    <>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>
            {t('标签')}
            <small className="settings-count">{tags.length}</small>
          </span>
          <button className="settings-reset" onClick={() => setEditing('new')}>
            <Plus size={11} />
            {t('添加标签')}
          </button>
        </div>
        {tags.map((tag) => (
          <div className="settings-row tag-settings-row" key={tag.name}>
            <div className="settings-row-label">
              <span className="tag-settings-dot" style={{ background: tagHex(tag) }} />
              <span>{tag.name}</span>
            </div>
            <div className="settings-row-control">
              <button
                className="icon-button tag-settings-action"
                title={t('编辑标签')}
                aria-label={t('编辑标签 {0}', tag.name)}
                onClick={() => setEditing(tag)}
              >
                <Pencil size={13} />
              </button>
              <button
                className="icon-button tag-settings-action tag-settings-delete"
                title={t('删除标签')}
                aria-label={t('删除标签 {0}', tag.name)}
                onClick={() => setDeleting(tag)}
              >
                <Trash2 size={13} />
              </button>
            </div>
          </div>
        ))}
      </div>

      {deleting && (
        <Modal open onClose={() => setDeleting(null)} title={t('删除标签')}>
          <p className="tag-delete-confirm">{t('确定删除标签“{0}”吗？', deleting.name)}</p>
          <div className="dialog-actions">
            <Button variant="ghost" onClick={() => setDeleting(null)}>
              {t('取消')}
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                update({ tags: tags.filter((item) => item.name !== deleting.name) })
                setDeleting(null)
              }}
            >
              {t('删除')}
            </Button>
          </div>
        </Modal>
      )}

      {editing !== null && (
        <CreateTagDialog
          existing={editing === 'new' ? tags : tags.filter((tag) => tag.name !== editing.name)}
          editing={editing === 'new' ? undefined : editing}
          onClose={() => setEditing(null)}
          onCreate={(tag) => {
            if (editing === 'new') update({ tags: [...tags, tag] })
            else
              update({
                tags: tags.map((item) => (item.name === editing.name ? tag : item)),
              })
            setEditing(null)
          }}
        />
      )}
    </>
  )
}
