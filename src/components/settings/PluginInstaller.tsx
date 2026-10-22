import { useState } from 'react'
import { Download, Github, Shield } from 'lucide-react'
import { call } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'

export const permissionTitles: Record<string, string> = {
  storage: '插件数据存储',
  secrets: '系统凭据库（密码与 OAuth 凭据）',
  network: '访问网络',
  'filesystem.read': '读取文件',
  'filesystem.write': '修改文件',
  externalCommands: '执行外部程序',
  'ui.dialog': '显示文件选择器',
}
interface InstallPreview {
  ticket: string
  id: string
  name: string
  version: string
  author?: { name: string; url?: string }
  permissions: string[]
  repository: string
}

/**
 * Two-step installation: downloaded packages remain inert in a temporary
 * staging directory until user confirms the exact permission declaration.
 * No plugin executable is started during preview.
 */
export function PluginInstaller({ onChanged }: { onChanged: () => void }) {
  const [repository, setRepository] = useState('')
  const [busy, setBusy] = useState(false)
  const [assets, setAssets] = useState<string[]>([])
  const [preview, setPreview] = useState<InstallPreview | null>(null)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const loadAssets = async () => {
    if (busy || !repository.trim()) return
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const names = await call<string[]>('plugin_release_assets', {
        repository: repository.trim(),
      })
      setAssets(names)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  const prepare = async (assetName: string) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      const data = await call<InstallPreview>('plugin_prepare_install', {
        repository: repository.trim(),
        assetName,
      })
      setPreview(data)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  const discard = () => {
    const ticket = preview?.ticket
    setPreview(null)
    if (ticket) void call('plugin_cancel_install', { ticket }).catch(() => {})
  }
  const confirm = async () => {
    if (!preview || busy) return
    setBusy(true)
    setError('')
    try {
      await call('plugin_confirm_install', { ticket: preview.ticket })
      setPreview(null)
      setRepository('')
      setAssets([])
      setMessage('插件已安装并授权。可在下方配置或撤销权限。')
      onChanged()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }
  return (
    <>
      <div className="plugin-install-box">
        <div className="plugin-install-title">
          <Download size={16} />
          <strong>从 GitHub 安装插件</strong>
        </div>
        <div className="plugin-install-row">
          <Github size={17} />
          <input
            value={repository}
            aria-label="GitHub 插件仓库"
            placeholder="开发者/仓库名称，例如 owner/fiex-plugin"
            onChange={(event) => {
              setRepository(event.target.value)
              setAssets([])
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') void loadAssets()
            }}
          />
          <Button
            disabled={busy || !repository.trim()}
            type="button"
            onClick={() => void loadAssets()}
          >
            {busy ? '获取中…' : '获取插件'}
          </Button>
        </div>
        <p className="sidebar-hint">
          只支持 GitHub 最新 Release 中的 .fiexplugin
          文件。下载后先检查插件信息和权限，再由你确认安装。
        </p>
        {assets.length > 0 && (
          <div className="plugin-install-assets" aria-label="可安装插件">
            {assets.map((asset) => (
              <button type="button" key={asset} disabled={busy} onClick={() => void prepare(asset)}>
                <Download size={15} />
                <span>{asset}</span>
                <strong>查看权限并安装</strong>
              </button>
            ))}
          </div>
        )}
        {message && <p role="status">{message}</p>}
        {error && !preview && (
          <p role="alert" className="connection-error">
            {error}
          </p>
        )}
      </div>
      <Modal open={Boolean(preview)} title="确认插件安装及权限" onClose={discard}>
        <div className="plugin-install-review">
          <div className="plugin-install-review-heading">
            <Shield size={21} />
            <div>
              <strong>{preview?.name}</strong>
              <small>
                {preview?.id} · {preview?.version}
              </small>
            </div>
          </div>
          <div className="plugin-install-meta">
            <span>作者：{preview?.author?.name || '未知开发者'}</span>
            <span>来源：GitHub / {preview?.repository}</span>
          </div>
          <strong>请求的权限</strong>
          <div className="plugin-install-permissions">
            {preview?.permissions.length ? (
              preview.permissions.map((permission) => (
                <div key={permission}>
                  <Shield size={14} />
                  {permissionTitles[permission] || permission}
                </div>
              ))
            ) : (
              <span>无需宿主权限</span>
            )}
          </div>
          <p className="sidebar-hint">
            所有插件（包括 Fiex 官方发布的插件）使用同样的授权流程。插件为 Native
            程序；在系统沙箱完善前，权限控制仅约束宿主代理的操作，不能阻止插件直接进行系统调用。
          </p>
          {error && (
            <p role="alert" className="connection-error">
              {error}
            </p>
          )}
          <footer className="plugin-consent-actions">
            <Button variant="outline" type="button" disabled={busy} onClick={discard}>
              取消
            </Button>
            <Button type="button" disabled={busy} onClick={() => void confirm()}>
              {busy ? '安装中…' : '同意权限并安装'}
            </Button>
          </footer>
        </div>
      </Modal>
    </>
  )
}
