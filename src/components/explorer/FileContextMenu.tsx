import type { AppController } from '@/hooks/useAppController'
import { call } from '@/lib/api'
import { archiveFormats } from '@/lib/archives'
import { isArchive } from '@/lib/files'
import {
  Archive,
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
interface ContextAction {
  disabled?: boolean
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
  gitSwitched,
  openTags,
  x,
  isRemotePath,
  run,
  copy,
  duplicate,
  compress,
  extract,
  terminal,
  notify,
  recycle,
  deletePermanently,
  inTrash,
  setCompressOpen,
  compressOpen,
  setMenu,
}: Pick<
  AppController,
  | 'menu'
  | 'hint'
  | 'open'
  | 'previewEntry'
  | 'setInfo'
  | 'platform'
  | 'gitSwitched'
  | 'openTags'
  | 'x'
  | 'isRemotePath'
  | 'run'
  | 'copy'
  | 'duplicate'
  | 'compress'
  | 'extract'
  | 'terminal'
  | 'notify'
  | 'recycle'
  | 'deletePermanently'
  | 'inTrash'
  | 'setCompressOpen'
  | 'compressOpen'
  | 'setMenu'
>) {
  return (
    <>
      {menu && (
        <div
          role="menu"
          aria-label="文件操作"
          className="context-menu"
          style={{
            left: Math.min(menu.x, innerWidth - 360),
            top: Math.max(8, Math.min(menu.y, innerHeight - 380)),
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
                      disabled: gitSwitched || x.directoryReadOnly,
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
                      action: () =>
                        void run('正在同步…', async () => {
                          const paths = x.selected.includes(menu.entry.path)
                            ? x.selected
                            : [menu.entry.path]
                          await call(
                            menu.entry.sync === 'synced' ? 'remote_unsync' : 'remote_sync',
                            { paths },
                          )
                          x.reload()
                        }),
                    },
                  ]
                : []),
              {
                label: '重新命名',
                icon: <Pencil />,
                hint: hint('rename'),
                disabled: gitSwitched || x.directoryReadOnly,
                action: () => x.beginRename(menu.entry.path),
              },
              { label: '复制', icon: <Copy />, hint: hint('copy'), action: () => copy(false) },
              {
                label: '剪切',
                icon: <Scissors />,
                hint: hint('cut'),
                disabled: gitSwitched || x.directoryReadOnly,
                action: () => copy(true),
              },
              {
                label: '创建副本',
                icon: <Copy />,
                hint: hint('duplicate'),
                disabled: gitSwitched || x.directoryReadOnly,
                action: duplicate,
              },
              {
                label: '压缩为…',
                icon: <Archive />,
                hint: '',
                disabled: gitSwitched || x.directoryReadOnly,
                submenu: archiveFormats.map((format) => ({
                  label: format.name,
                  action: () => compress(format),
                })),
              },
              ...(isArchive(menu.entry)
                ? [
                    {
                      label: '解压缩',
                      icon: <Archive />,
                      hint: '',
                      disabled: gitSwitched || x.directoryReadOnly,
                      action: extract,
                    },
                  ]
                : []),
              {
                label: 'Open in Terminal',
                icon: <TerminalSquare />,
                hint: '',
                disabled: gitSwitched || x.directoryReadOnly,
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
                    label: '彻底删除',
                    icon: <Trash2 />,
                    hint: '',
                    action: () =>
                      deletePermanently(
                        x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path],
                      ),
                  }
                : {
                    label: '移到废纸篓',
                    icon: <Trash2 />,
                    hint: hint('trash'),
                    disabled: gitSwitched || x.directoryReadOnly,
                    action: recycle,
                  },
            ] as ContextItem[]
          ).map((a) =>
            'submenu' in a ? (
              <div
                className="context-submenu-wrap"
                key={a.label}
                onMouseEnter={() => setCompressOpen(true)}
                onMouseLeave={() => setCompressOpen(false)}
              >
                <button
                  role="menuitem"
                  disabled={a.disabled}
                  aria-haspopup="menu"
                  aria-expanded={compressOpen}
                  onClick={(event) => {
                    event.stopPropagation()
                    setCompressOpen((value) => !value)
                  }}
                >
                  {a.icon}
                  {a.label}
                  <ChevronRight className="submenu-arrow" size={13} />
                </button>
                {compressOpen && !a.disabled && (
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
        </div>
      )}
    </>
  )
}
