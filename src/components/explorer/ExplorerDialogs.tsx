import { ConnectionDialog } from '@/components/ConnectionDialog'
import { FileInfoDialog } from '@/components/FileInfoDialog'
import { PreferencesDialog } from '@/components/PreferencesDialog'
import { Preview } from '@/components/Preview'
import { TagDialog } from '@/components/TagDialog'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import type { AppController } from '@/hooks/useAppController'
import { call } from '@/lib/api'
import { CheckCircle2, LoaderCircle, X } from 'lucide-react'
export function ExplorerDialogs({
  preview,
  setPreview,
  navigatePreview,
  open,
  dialog,
  dialogBusy,
  setDialog,
  setDialogBusy,
  setDialogError,
  dialogValue,
  x,
  setDialogValue,
  dialogError,
  trashIssue,
  setTrashIssue,
  openTrash,
  info,
  setInfo,
  tagOpen,
  tagInitial,
  setTagOpen,
  applyTags,
  connectionOpen,
  setConnectionOpen,
  notify,
  settings,
  setSettings,
  prefs,
  update,
  terminals,
  ides,
  toast,
  setToast,
}: Pick<
  AppController,
  | 'preview'
  | 'setPreview'
  | 'navigatePreview'
  | 'open'
  | 'dialog'
  | 'dialogBusy'
  | 'setDialog'
  | 'setDialogBusy'
  | 'setDialogError'
  | 'dialogValue'
  | 'x'
  | 'setDialogValue'
  | 'dialogError'
  | 'trashIssue'
  | 'setTrashIssue'
  | 'openTrash'
  | 'info'
  | 'setInfo'
  | 'tagOpen'
  | 'tagInitial'
  | 'setTagOpen'
  | 'applyTags'
  | 'connectionOpen'
  | 'setConnectionOpen'
  | 'notify'
  | 'settings'
  | 'setSettings'
  | 'prefs'
  | 'update'
  | 'terminals'
  | 'ides'
  | 'toast'
  | 'setToast'
>) {
  return (
    <>
      <Preview
        entry={preview}
        onClose={() => setPreview(null)}
        onNavigate={navigatePreview}
        onOpen={open}
      />

      <Modal
        open={!!dialog}
        onClose={() => {
          if (!dialogBusy) setDialog(null)
        }}
        title={dialog?.title || ''}
        description={dialog?.description}
      >
        <form
          onSubmit={async (e) => {
            e.preventDefault()
            if (!dialog || dialogBusy) return
            setDialogBusy(true)
            setDialogError('')
            try {
              await dialog.action(dialogValue)
              setDialog(null)
            } catch (err) {
              setDialogError(String(err))
              x.reload()
            } finally {
              setDialogBusy(false)
            }
          }}
        >
          {dialog?.label && (
            <label className="field-label">
              {dialog.label}
              <input
                className="text-field"
                autoFocus
                value={dialogValue}
                onFocus={(e) => e.target.select()}
                onChange={(e) => setDialogValue(e.target.value)}
              />
            </label>
          )}
          {dialogError && (
            <p className="error" role="alert">
              {dialogError}
            </p>
          )}
          <div className="dialog-actions">
            <Button
              type="button"
              variant="ghost"
              disabled={dialogBusy}
              onClick={() => setDialog(null)}
            >
              取消
            </Button>
            <Button
              type="submit"
              variant={dialog?.danger ? 'destructive' : 'default'}
              disabled={dialogBusy || (!!dialog?.label && !dialogValue.trim())}
            >
              {dialogBusy && <LoaderCircle className="spin" />}
              {dialogBusy ? '正在处理…' : dialog?.submit}
            </Button>
          </div>
        </form>
      </Modal>

      <Modal
        open={trashIssue}
        onClose={() => setTrashIssue(false)}
        title="无法浏览废纸篓"
        description="macOS 不允许应用直接读取 ~/.Trash。请在“系统设置 → 隐私与安全性 → 完全磁盘访问权限”中允许 Fiex；授权后如仍无法打开，请重启 Fiex。也可以现在用 Finder 打开。"
      >
        <div className="dialog-actions">
          <Button variant="ghost" onClick={() => setTrashIssue(false)}>
            取消
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              setTrashIssue(false)
              void call('open_trash').catch((error) => x.setError(String(error)))
            }}
          >
            在 Finder 中打开
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              setTrashIssue(false)
              setTimeout(() => void openTrash(), 300)
            }}
          >
            重试
          </Button>
          <Button
            onClick={() => {
              setTrashIssue(false)
              void call('open_full_disk_access').catch((error) => x.setError(String(error)))
            }}
          >
            打开权限设置
          </Button>
        </div>
      </Modal>

      <FileInfoDialog
        info={info}
        gitStatus={info ? x.git?.entries[info.path] : undefined}
        onClose={() => setInfo(null)}
        onError={x.setError}
      />

      <TagDialog
        open={tagOpen}
        initial={tagInitial}
        onClose={() => setTagOpen(false)}
        onApply={applyTags}
      />

      <ConnectionDialog
        open={connectionOpen}
        onClose={() => setConnectionOpen(false)}
        onSaved={(connection) => {
          setConnectionOpen(false)
          void x.refreshConnections()
          x.navigate(connection.mountPath)
          notify(`已连接 ${connection.name}`)
        }}
      />

      <PreferencesDialog
        open={settings}
        onClose={() => setSettings(false)}
        prefs={prefs}
        locations={x.locations}
        update={update}
        terminals={terminals}
        ides={ides}
        onError={x.setError}
      />

      {toast && (
        <div className="toast" role="status">
          <CheckCircle2 size={16} />
          {toast}
          <button title="关闭通知" onClick={() => setToast('')}>
            <X size={13} />
          </button>
        </div>
      )}
    </>
  )
}
