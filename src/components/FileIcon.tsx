import { useEffect, useState } from 'react'
import {
  AppWindow,
  Archive,
  File,
  FileCode2,
  FileImage,
  FileMusic,
  FileSpreadsheet,
  FileText,
  FileVideo,
  Folder,
  Link2,
  Presentation,
  Type,
} from 'lucide-react'
import { call } from '@/lib/api'
import { kind } from '@/lib/files'
import type { Entry } from '@/lib/types'
const cache = new Map<string, Promise<string | null>>()
export function FileIcon({ entry: e, size = 28 }: { entry: Entry; size?: number }) {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    const key = e.path + ':' + e.modified
    if (!cache.has(key)) {
      if (cache.size > 2000) cache.clear()
      cache.set(
        key,
        call<string>('native_icon', { path: e.path }).catch(() => null),
      )
    }
    void cache.get(key)!.then((v) => {
      if (active) setUrl(v)
    })
    return () => {
      active = false
    }
  }, [e.path, e.modified])
  const k = kind(e)
  const Icon =
    k === '应用程序'
      ? AppWindow
      : e.isDir
        ? Folder
        : k === '图片'
          ? FileImage
          : k === '归档'
            ? Archive
            : k === '视频'
              ? FileVideo
              : k === '音频'
                ? FileMusic
                : k === '表格'
                  ? FileSpreadsheet
                  : k === '文档'
                    ? FileText
                    : k === '演示文稿'
                      ? Presentation
                      : k === '字体'
                        ? Type
                        : k === '文本与代码'
                          ? FileCode2
                          : File
  return (
    <span
      className={`file-icon ${e.isDir ? 'folder-icon' : ''}`}
      style={{ width: size, height: size }}
    >
      {url ? (
        <img src={url} alt="" draggable={false} />
      ) : (
        <Icon
          size={size}
          strokeWidth={1.3}
          fill={e.isDir ? 'currentColor' : 'none'}
          fillOpacity={e.isDir ? 0.2 : 0}
        />
      )}
      {e.isSymlink && <Link2 className="symlink" size={12} />}
    </span>
  )
}
