import { call } from '@/lib/api'
import { progressChannel } from '@/hooks/useActivity'
import {
  CloudCheck,
  CloudDownload,
  Copy,
  ExternalLink,
  Eye,
  Info,
  Pencil,
  RotateCcw,
  Scissors,
  Tags,
  TerminalSquare,
  Trash2,
} from 'lucide-react'
import type { ContextItem, ContextItemsProps } from './types'

/** Builds the file menu: opening, syncing, clipboard, rename, terminal and the
 *  delete/restore pair that depends on the Trash. */
export function contextItems({
  menu,
  hint,
  open,
  previewEntry,
  setInfo,
  openBatchRename,
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
}: ContextItemsProps): ContextItem[] {
  return [
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
              openTags(x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path]),
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
              const paths = x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path]
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
    ...(x.selected.length > 1
      ? [
          {
            label: '批量重命名…',
            icon: <Pencil />,
            hint: '',
            disabled: x.directoryReadOnly,
            action: openBatchRename,
          },
        ]
      : []),
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
            restoreFromTrash(x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path]),
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
}
