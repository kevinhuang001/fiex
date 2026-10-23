import { useState } from 'react'
import type { AppController } from '@/hooks/useAppController'
import { progressChannel } from '@/hooks/useActivity'
import { call } from '@/lib/api'
import {
  RotateCcw,
  ChevronRight,
  CloudCheck,
  CloudDownload,
  Copy,
  ExternalLink,
  Eye,
  Info,
  Pencil,
  Scissors,
  Tags,
  TerminalSquare,
  Trash2,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { PluginContextActions } from '@/components/plugins/PluginContextActions'
interface ContextAction {
  disabled?: boolean
  danger?: boolean
  label: string
  icon: ReactNode
  hint: string
  action: () => void
}
interface ContextSubmenu {
  disabled?: boolean
  label: string
  icon: ReactNode
  hint: string
  submenu: { label: string; action: () => void }[]
}
type ContextItem = ContextAction | ContextSubmenu
export function FileContextMenu({
  menu,
  hint,
  open,
  previewEntry,
  setInfo,
  platform,
  openTags,
  x,
  isRemotePath,
  run,
  copy,
  duplicate,
  terminal,
  notify,
  recycle,
  deletePermanently,
  restoreFromTrash,
  inTrash,
  setMenu,
}: Pick<
  AppController,
  | 'menu'
  | 'hint'
  | 'open'
  | 'previewEntry'
  | 'setInfo'
  | 'platform'
  | 'openTags'
  | 'x'
  | 'isRemotePath'
  | 'run'
  | 'copy'
  | 'duplicate'
  | 'terminal'
  | 'notify'
  | 'recycle'
  | 'deletePermanently'
  | 'restoreFromTrash'
  | 'inTrash'
  | 'setMenu'
>) {
  const [submenuOpen, setSubmenuOpen] = useState(false)
  return (
    <>
      {menu && (
        <div
          role="menu"
          aria-label="文件操作"
          className="context-menu"
          style={{
            left: Math.min(menu.x, innerWidth - 360),
            top: Math.max(8, Math.min(menu.y, innerHeight - 440)),
          }}
        >
          {(
            [
              {
                label: '打开',
                icon: <ExternalLink />,
                hint: hint('open'),
                action: () => open(menu.entry),
              },
              {
                label: '快速查看',
                icon: <Eye />,
                hint: hint('quickLook'),
                action: () => previewEntry(menu.entry),
              },
              {
                label: '显示简介',
                icon: <Info />,
                hint: hint('info'),
                action: () => setInfo(menu.entry),
              },
              ...(platform === 'macos'
                ? [
                    {
                      label: '标签…',
                      disabled: x.directoryReadOnly,
                      icon: <Tags />,
                      hint: '',
                      action: () =>
                        openTags(
                          x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path],
                        ),
                    },
                  ]
                : []),
              ...(isRemotePath(menu.entry.path)
                ? [
                    {
                      label: menu.entry.sync === 'synced' ? '取消同步' : '同步',
                      icon: menu.entry.sync === 'synced' ? <CloudCheck /> : <CloudDownload />,
                      hint: '',
                      action: () => {
                        const paths = x.selected.includes(menu.entry.path)
                          ? x.selected
                          : [menu.entry.path]
                        const synced = menu.entry.sync === 'synced'
                        let onProgress: ReturnType<typeof progressChannel> | undefined
                        void run(
                          synced ? '正在取消同步…' : '正在同步…',
                          async () => {
                            await call(
                              synced ? 'remote_unsync' : 'remote_sync',
                              synced ? { paths } : { paths, onProgress },
                            )
                            x.reload()
                          },
                          `${paths.length} 个项目`,
                          false,
                          'sync',
                          (id) => {
                            if (!synced) onProgress = progressChannel(id)
                          },
                        )
                      },
                    },
                  ]
                : []),
              {
                label: '重新命名',
                icon: <Pencil />,
                hint: hint('rename'),
                disabled: x.directoryReadOnly,
                action: () => x.beginRename(menu.entry.path),
              },
              { label: '复制', icon: <Copy />, hint: hint('copy'), action: () => copy(false) },
              {
                label: '剪切',
                icon: <Scissors />,
                hint: hint('cut'),
                disabled: x.directoryReadOnly,
                action: () => copy(true),
              },
              {
                label: '创建副本',
                icon: <Copy />,
                hint: hint('duplicate'),
                disabled: x.directoryReadOnly,
                action: duplicate,
              },
              {
                label: 'Open in Terminal',
                icon: <TerminalSquare />,
                hint: '',
                disabled: x.directoryReadOnly,
                action: () => terminal(),
              },
              {
                label: '复制路径',
                icon: <Copy />,
                hint: '',
                action: () =>
                  void navigator.clipboard
                    .writeText(menu.entry.path)
                    .then(() => notify('路径已复制'))
                    .catch((e) => x.setError(String(e))),
              },
              inTrash
                ? {
                    label: '放回原处',
                    icon: <RotateCcw />,
                    hint: '',
                    action: () =>
                      restoreFromTrash(
                        x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path],
                      ),
                  }
                : {
                    label: '移到废纸篓',
                    icon: <Trash2 />,
                    hint: hint('trash'),
                    disabled: x.directoryReadOnly,
                    danger: true,
                    action: recycle,
                  },
              ...(inTrash
                ? [
                    {
                      label: '彻底删除',
                      icon: <Trash2 />,
                      hint: '',
                      danger: true,
                      action: () =>
                        deletePermanently(
                          x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path],
                        ),
                    },
                  ]
                : []),
            ] as ContextItem[]
          ).map((a) =>
            'submenu' in a ? (
              <div
                className="context-submenu-wrap"
                key={a.label}
                onMouseEnter={() => setSubmenuOpen(true)}
                onMouseLeave={() => setSubmenuOpen(false)}
              >
                <button
                  role="menuitem"
                  disabled={a.disabled}
                  aria-haspopup="menu"
                  aria-expanded={submenuOpen}
                  onClick={(event) => {
                    event.stopPropagation()
                    setSubmenuOpen((value) => !value)
                  }}
                >
                  {a.icon}
                  {a.label}
                  <ChevronRight className="submenu-arrow" size={13} />
                </button>
                {submenuOpen && !a.disabled && (
                  <div className="context-submenu" role="menu">
                    {a.submenu.map((item) => (
                      <button
                        role="menuitem"
                        key={item.label}
                        onClick={() => {
                          item.action()
                          setMenu(null)
                        }}
                      >
                        {item.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ) : (
              <button
                role="menuitem"
                key={a.label}
                disabled={a.disabled}
                className={a.danger ? 'context-danger' : undefined}
                onClick={() => {
                  a.action()
                  setMenu(null)
                }}
              >
                {a.icon}
                {a.label}
                {a.hint && <span className="shortcut">{a.hint}</span>}
              </button>
            ),
          )}
          <PluginContextActions
            paths={x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path]}
            path={x.path}
            isDirectory={menu.entry.isDir}
            readOnly={x.readOnly}
            isRemote={isRemotePath(menu.entry.path)}
            onClose={() => setMenu(null)}
            onError={x.setError}
          />
        </div>
      )}
    </>
  )
}
