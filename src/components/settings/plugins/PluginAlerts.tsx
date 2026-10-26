import { Puzzle, ShieldAlert, ShieldCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import type { PluginInfo } from '@/lib/plugins'

/** Preview returned by the host before a local `.fiexplugin` file is confirmed. */
export interface LocalPreview {
  ticket: string
  id: string
  name: string
  version: string
  author?: { name: string }
  permissions: string[]
  requestedNetworkDomains: string[]
  commands: string[]
  signed: boolean
  trusted: boolean
  publisher?: string
  publisherName?: string
  keyId?: string
  fingerprint?: string
}

/** Inline status line for import/install results and host errors. */
export function PluginMessage({ message }: { message: string }) {
  if (!message) return null
  return (
    <p role="status" className="plugin-page-message">
      {message}
    </p>
  )
}

/** Uninstall confirmation: plugin data and synced user directories are kept. */
export function PluginUninstallDialog({
  plugin,
  busy,
  onClose,
  onConfirm,
}: {
  plugin: PluginInfo | null
  busy: boolean
  onClose: () => void
  onConfirm: () => void
}) {
  return (
    <Modal open={Boolean(plugin)} title="卸载插件" onClose={onClose}>
      <p>
        确定卸载「{plugin?.name}
        」吗？将停止插件进程并删除程序文件，插件数据和用户同步目录会保留。
      </p>
      <footer className="plugin-consent-actions">
        <Button variant="outline" onClick={onClose}>
          取消
        </Button>
        <Button variant="destructive" disabled={busy} onClick={() => void onConfirm()}>
          卸载
        </Button>
      </footer>
    </Modal>
  )
}

/** Local package preview before the import is confirmed. */
export function PluginImportDialog({
  preview,
  busy,
  onClose,
  onCancel,
  onConfirm,
}: {
  preview: LocalPreview | null
  busy: boolean
  onClose: () => void
  onCancel: () => void
  onConfirm: () => void
}) {
  return (
    <Modal
      open={Boolean(preview)}
      title="导入插件"
      onClose={onClose}
      className="plugin-import-modal"
    >
      {preview && (
        <div className="plugin-import-page">
          <div className="plugin-import-profile">
            <span className="plugin-import-icon">
              <Puzzle size={24} />
            </span>
            <span className="plugin-import-copy">
              <strong>{preview.name}</strong>
              <small>{preview.author?.name || '未知开发者'}</small>
            </span>
          </div>
          <div className={`plugin-import-signature ${preview.signed ? 'verified' : 'warning'}`}>
            {preview.signed ? <ShieldCheck size={16} /> : <ShieldAlert size={16} />}
            <span>
              <strong>{preview.signed ? '插件包已签名' : '插件包未签名'}</strong>
              {preview.signed && (
                <>
                  签名者：
                  {preview.publisherName || preview.publisher || '未知签名者'}
                  {(preview.fingerprint || preview.keyId) && (
                    <>
                      <br />
                      密钥指纹：<code>{preview.fingerprint || preview.keyId}</code>
                    </>
                  )}
                  <br />
                </>
              )}
              <em>Core 内置 Ed25519 校验 · 请只导入你信任来源的插件，并确认它将获得的权限。</em>
            </span>
          </div>
          <footer className="plugin-consent-actions">
            <Button variant="outline" disabled={busy} onClick={onCancel}>
              取消
            </Button>
            <Button disabled={busy} onClick={() => void onConfirm()}>
              导入插件
            </Button>
          </footer>
        </div>
      )}
    </Modal>
  )
}
