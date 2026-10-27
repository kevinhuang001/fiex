import { t } from '@/i18n'
import { call } from '@/lib/api'
import { progressChannel } from '@/hooks/useActivity'
import {
  CloudCheck,
  CloudDownload,
  CloudOff,
  Copy,
  ExternalLink,
  Eye,
  Info,
  Pencil,
  PinOff,
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
  const multiple = x.selected.length > 1
  return [
    {
      label: t('打开'),
      icon: <ExternalLink />,
      hint: hint('open'),
      action: () => open(menu.entry),
    },
    ...(multiple
      ? []
      : [
          {
            label: t('快速查看'),
            icon: <Eye />,
            hint: hint('quickLook'),
            action: () => previewEntry(menu.entry),
          },
          {
            label: t('显示简介'),
            icon: <Info />,
            hint: hint('info'),
            action: () => setInfo(menu.entry),
          },
        ]),
    ...(platform === 'macos'
      ? [
          {
            label: t('标签…'),
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
            label: menu.entry.pinned ? t('取消保留在本地') : t('始终保留在本地'),
            icon: menu.entry.pinned ? <PinOff /> : <CloudDownload />,
            hint: '',
            action: () => {
              const paths = x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path]
              if (menu.entry.pinned) {
                void run(t('正在取消保留…'), async () => {
                  await call('remote_unpin', { paths })
                  x.reload()
                })
                return
              }
              const connection = x.connections.find(
                (candidate) =>
                  menu.entry.path === candidate.mountPath ||
                  menu.entry.path.startsWith(candidate.mountPath.replace(/\/$/, '') + '/'),
              )
              const label = connection?.name ? t('{0} 触发同步', connection.name) : t('触发同步')
              let onProgress: ReturnType<typeof progressChannel> | undefined
              void run(
                label,
                async () => {
                  await call('remote_sync', { paths, onProgress })
                  x.reload()
                },
                t('{0} 个项目', paths.length),
                false,
                'sync',
                (id) => {
                  onProgress = progressChannel(id)
                },
                // Pin downloads are recorded but never open the panel.
                true,
              )
            },
          },
          ...(menu.entry.hasCache
            ? menu.entry.pinned || menu.entry.sync === 'conflict'
              ? []
              : [
                  {
                    label: t('取消缓存'),
                    icon: <CloudOff />,
                    hint: '',
                    action: () => {
                      const paths = x.selected.includes(menu.entry.path)
                        ? x.selected
                        : [menu.entry.path]
                      // Uncaching is immediate bookkeeping; keep it out of the
                      // activity centre and only surface real failures.
                      void call('remote_uncache', { paths })
                        .then(() => x.reload())
                        .catch((error) => x.setError(String(error)))
                    },
                  },
                ]
            : [
                {
                  label: t('缓存到本地'),
                  icon: <CloudDownload />,
                  hint: '',
                  action: () => {
                    const paths = x.selected.includes(menu.entry.path)
                      ? x.selected
                      : [menu.entry.path]
                    let onProgress: ReturnType<typeof progressChannel> | undefined
                    void run(
                      t('正在缓存…'),
                      async () => {
                        await call('remote_cache', { paths, onProgress })
                      },
                      t('{0} 个项目', paths.length),
                      true,
                      'transfer',
                      (id) => {
                        onProgress = progressChannel(id)
                      },
                    )
                  },
                },
              ]),
          ...(menu.entry.sync === 'conflict'
            ? [
                ...[true, false].map((keepLocal) => ({
                  label: keepLocal ? t('用本地版本覆盖云端') : t('保留本地备份并采用云端版本'),
                  icon: <CloudCheck />,
                  hint: '',
                  action: () => {
                    void run(t('正在处理同步冲突…'), async () => {
                      await call('remote_resolve', { path: menu.entry.path, keepLocal })
                      x.reload()
                    })
                  },
                })),
              ]
            : []),
        ]
      : []),
    ...(multiple
      ? []
      : [
          {
            label: t('重新命名'),
            icon: <Pencil />,
            hint: hint('rename'),
            disabled: x.directoryReadOnly,
            action: () => x.beginRename(menu.entry.path),
          },
        ]),
    ...(x.selected.length > 1
      ? [
          {
            label: t('批量重命名…'),
            icon: <Pencil />,
            hint: '',
            disabled: x.directoryReadOnly,
            action: openBatchRename,
          },
        ]
      : []),
    { label: t('复制'), icon: <Copy />, hint: hint('copy'), action: () => copy(false) },
    {
      label: t('剪切'),
      icon: <Scissors />,
      hint: hint('cut'),
      disabled: x.directoryReadOnly,
      action: () => copy(true),
    },
    {
      label: t('创建副本'),
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
      label: t('复制路径'),
      icon: <Copy />,
      hint: '',
      action: () =>
        void navigator.clipboard
          .writeText(menu.entry.path)
          .then(() => notify(t('路径已复制')))
          .catch((e) => x.setError(String(e))),
    },
    inTrash
      ? {
          label: t('放回原处'),
          icon: <RotateCcw />,
          hint: '',
          action: () =>
            restoreFromTrash(x.selected.includes(menu.entry.path) ? x.selected : [menu.entry.path]),
        }
      : {
          label: t('移到废纸篓'),
          icon: <Trash2 />,
          hint: hint('trash'),
          disabled: x.directoryReadOnly,
          danger: true,
          action: recycle,
        },
    ...(inTrash
      ? [
          {
            label: t('彻底删除'),
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
