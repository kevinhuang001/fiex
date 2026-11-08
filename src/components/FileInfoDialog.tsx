import { t } from '@/i18n'
import { Check, Copy } from 'lucide-react'
import { Fragment, useEffect, useRef, useState } from 'react'
import { Modal } from './ui/Modal'
import { FileIcon } from './FileIcon'
import { date, displayAddress, isVirtualResource, parent, kind, size } from '@/lib/files'
import { tagHex } from '@/lib/tags'
import { call, native } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import type { Entry } from '@/lib/types'
import { Channel } from '@tauri-apps/api/core'

interface InfoRow {
  label: string
  value: string
  copyable?: boolean
  action?: string
}
interface InfoSection {
  id: string
  source: 'core' | 'plugin'
  title: string
  pluginId?: string
  provider: string
  rows: InfoRow[]
}

export function FileInfoDialog({ info, onClose }: { info: Entry | null; onClose: () => void }) {
  const [sections, setSections] = useState<InfoSection[]>([])
  const [loading, setLoading] = useState(false)
  const [copied, setCopied] = useState('')
  const copiedTimer = useRef<number | null>(null)
  const [folderSize, setFolderSize] = useState<{
    path: string
    value?: number
    loading: boolean
  }>()
  const folderSizeRequest = useRef<{ path: string; id: string } | null>(null)
  const allowedOpsKey = info?.allowedOps?.join('\0') ?? ''

  useEffect(
    () => () => {
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
    },
    [],
  )

  useEffect(() => {
    if (!info) return
    let alive = true
    setSections([])
    setLoading(true)
    const onPartial = native ? new Channel<InfoSection[]>() : undefined
    if (onPartial) {
      onPartial.onmessage = (partial) => {
        if (!alive || !Array.isArray(partial)) return
        setSections((previous) => {
          const sections = new Map(previous.map((section) => [section.id, section]))
          for (const section of partial) sections.set(section.id, section)
          return [...sections.values()]
        })
      }
    }
    let timeoutId: number | undefined
    const timeout = new Promise<InfoSection[]>((_, reject) => {
      timeoutId = window.setTimeout(
        () => reject(new Error(t('读取插件信息超时，请检查插件是否正常运行。'))),
        15_000,
      )
    })
    void Promise.race([
      call<InfoSection[]>('file_info_details', {
        path: info.path,
        ...(onPartial ? { onPartial } : {}),
      }),
      timeout,
    ])
      .then((value) => {
        if (alive) setSections(Array.isArray(value) ? value : [])
      })
      .catch((cause) => {
        if (alive) reportError(cause, 'file-info')
      })
      .finally(() => {
        if (timeoutId !== undefined) window.clearTimeout(timeoutId)
        if (alive) setLoading(false)
      })
    return () => {
      alive = false
      if (timeoutId !== undefined) window.clearTimeout(timeoutId)
    }
  }, [
    info?.path,
    info?.name,
    info?.isDir,
    info?.isSymlink,
    info?.size,
    info?.modified,
    info?.mime,
    allowedOpsKey,
  ])

  useEffect(() => {
    setFolderSize(undefined)
    return () => {
      const request = folderSizeRequest.current
      if (request && request.path === info?.path) {
        folderSizeRequest.current = null
        void call('file_info_directory_size_cancel', {
          path: request.path,
          requestId: request.id,
        }).catch(() => {})
      }
    }
  }, [info?.path])

  const calculateFolderSize = async () => {
    if (!info?.isDir || info.isSymlink || isVirtualResource(info.path) || folderSizeRequest.current)
      return
    const request = { path: info.path, id: crypto.randomUUID() }
    folderSizeRequest.current = request
    setFolderSize({ path: request.path, loading: true })
    try {
      const value = await call<number>('file_info_directory_size', {
        path: request.path,
        requestId: request.id,
      })
      if (folderSizeRequest.current?.id === request.id)
        setFolderSize({ path: request.path, value, loading: false })
    } catch (cause) {
      if (folderSizeRequest.current?.id === request.id) {
        setFolderSize({ path: request.path, loading: false })
        reportError(cause, 'file-info:directory-size')
      }
    } finally {
      if (folderSizeRequest.current?.id === request.id) folderSizeRequest.current = null
    }
  }

  const copy = async (value: string, key: string) => {
    try {
      await navigator.clipboard.writeText(value)
      setCopied(key)
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current)
      copiedTimer.current = window.setTimeout(() => setCopied(''), 1200)
    } catch {
      // Clipboard access is best effort; the value stays selectable.
    }
  }

  const currentFolderSize = info && folderSize?.path === info.path ? folderSize : undefined
  const basicRows: InfoRow[] = info
    ? [
        { label: t('种类'), value: kind(info) },
        { label: t('大小'), value: info.isDir ? t('文件夹') : size(info.size) },
        { label: t('位置'), value: displayAddress(parent(info.path)) },
        { label: t('创建时间'), value: date(info.created) },
        { label: t('修改时间'), value: date(info.modified) },
        { label: t('符号链接'), value: info.isSymlink ? t('是') : t('否') },
        ...(info.linkTarget ? [{ label: t('链接到'), value: info.linkTarget }] : []),
      ]
    : []
  // Core's metadata provider owns the basic attributes. Keep the Entry
  // fallback for resources without a resolved Core metadata section.
  const hasCoreMetadata = sections.some((section) => section.id === 'core.metadata')
  const groups: Array<{ title: string; from?: string; rows: InfoRow[] }> = info
    ? [
        ...(hasCoreMetadata ? [] : [{ title: t('文件'), rows: basicRows }]),
        ...(info.isDir && !info.isSymlink && !isVirtualResource(info.path)
          ? [
              {
                title: t('文件夹'),
                rows: [
                  {
                    label: t('文件夹大小'),
                    value:
                      currentFolderSize?.value === undefined ? '—' : size(currentFolderSize.value),
                    action:
                      currentFolderSize?.value === undefined
                        ? currentFolderSize?.loading
                          ? t('正在计算…')
                          : t('计算大小')
                        : undefined,
                  },
                ],
              },
            ]
          : []),
        ...sections
          .filter((section) => section.rows.length > 0)
          .map((section) => ({
            title: section.title,
            from:
              section.source === 'plugin' && section.provider
                ? t('来自：{0}', section.provider)
                : undefined,
            rows: section.rows,
          })),
      ]
    : []

  return (
    <Modal open={!!info} onClose={onClose} title={t('显示简介')} className="file-info-modal">
      {info && (
        <div className="file-info">
          <FileIcon entry={info} size={72} />
          <h3>{info.name}</h3>
          <div className="file-info-scroll">
            <dl>
              {groups.map((group, groupIndex) => (
                <Fragment key={`${group.title}:${groupIndex}`}>
                  <div className="file-info-group">
                    <span>{group.title}</span>
                    {group.from && <small>{group.from}</small>}
                  </div>
                  {group.rows.map((row, rowIndex) => (
                    <div className="file-info-row" key={`${row.label}:${rowIndex}`}>
                      <dt>{row.label}</dt>
                      <dd>
                        <span>{row.value}</span>
                        {row.copyable && (
                          <button
                            type="button"
                            className="info-copy"
                            title={copied === `${groupIndex}:${rowIndex}` ? t('已复制') : t('复制')}
                            aria-label={t('复制')}
                            onClick={() => void copy(row.value, `${groupIndex}:${rowIndex}`)}
                          >
                            {copied === `${groupIndex}:${rowIndex}` ? (
                              <Check size={12} />
                            ) : (
                              <Copy size={12} />
                            )}
                          </button>
                        )}
                        {row.action && (
                          <button
                            type="button"
                            className="file-info-action"
                            disabled={currentFolderSize?.loading}
                            onClick={() => void calculateFolderSize()}
                          >
                            {row.action}
                          </button>
                        )}
                      </dd>
                    </div>
                  ))}
                </Fragment>
              ))}
            </dl>
            {loading && <p className="file-info-status">{t('正在读取媒体与插件信息…')}</p>}
            {info.tags.length > 0 && (
              <div className="file-info-tags">
                {info.tags.map((tag) => (
                  <span className="tag-chip" key={`${tag.name}:${tag.color}`}>
                    <i style={{ background: tagHex(tag) }} />
                    {tag.name}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </Modal>
  )
}
