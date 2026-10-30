import { t } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { Link2 } from 'lucide-react'
import { call } from '@/lib/api'
import { kind } from '@/lib/files'
import { fileVisual } from '@/lib/fileIcons'
import { MemoryCache } from '@/lib/cache'
import { useDecorationIcon } from '@/lib/decorationIcons'
import { cachedPresentation } from '@/lib/entryPresentation'
import type { Entry } from '@/lib/types'

const cache = new MemoryCache<string | null>(600, 5 * 60_000)
// Bound native icon work: GTK icon extraction runs on the UI thread.
const iconQueue: (() => void)[] = []
let iconRequests = 0
function nativeIcon(path: string, size: number): Promise<string | null> {
  return new Promise((resolve) => {
    const start = () => {
      ++iconRequests
      void call<string>('native_icon', { path, size })
        .then(resolve, () => resolve(null))
        .finally(() => {
          --iconRequests
          iconQueue.shift()?.()
        })
    }
    if (iconRequests < 2) start()
    else iconQueue.push(start)
  })
}
const visibleCallbacks = new Map<Element, () => void>()
let observer: IntersectionObserver | undefined

function whenVisible(element: Element, callback: () => void) {
  observer ??= new IntersectionObserver(
    (entries) => {
      for (const item of entries)
        if (item.isIntersecting) {
          const run = visibleCallbacks.get(item.target)
          visibleCallbacks.delete(item.target)
          observer!.unobserve(item.target)
          run?.()
        }
    },
    { rootMargin: '150px' },
  )
  visibleCallbacks.set(element, callback)
  observer.observe(element)
  return () => {
    visibleCallbacks.delete(element)
    observer!.unobserve(element)
  }
}

/**
 * File-type icons come from the MIT-licensed Material Icon Theme. Native
 * application logos are still used for `.app`/`.exe`/`.desktop` bundles so
 * Fiex matches Finder where it matters.
 */
export function FileIcon({ entry: e, size = 28 }: { entry: Entry; size?: number }) {
  const isApplication = kind(e) === t('应用程序')
  // Virtual entries already carry their application icon through the decoration
  // response (built from the cached listing). native_icon would stat a URI that
  // has no host file, so keep it for real host paths only.
  const canNativeIcon = isApplication && !e.path.includes('://')
  const [native, setNative] = useState<{ key: string; url: string | null }>()
  const decorationIcon = useDecorationIcon(e)
  const ref = useRef<HTMLSpanElement>(null)
  const pixels = Math.min(
    512,
    Math.max(32, Math.ceil((size * (window.devicePixelRatio || 1)) / 32) * 32),
  )
  const iconKey = `${e.path}:${e.modified}:${pixels}`
  const nativeUrl = native?.key === iconKey ? native.url : cache.peek(iconKey)
  useEffect(() => {
    if (!canNativeIcon || !ref.current) return
    let active = true
    const stop = whenVisible(ref.current, () => {
      const key = iconKey
      void cache
        .get(key, () => nativeIcon(e.path, pixels))
        .then((url) => {
          if (active) setNative({ key, url })
        })
    })
    return () => {
      active = false
      stop()
    }
  }, [e.path, e.modified, canNativeIcon, pixels, iconKey])

  const visual = fileVisual(e)
  const generic = visual.generic ? (visual.folder ? 'generic-folder' : 'generic-file') : ''
  return (
    <span ref={ref} className="file-icon" style={{ width: size, height: size }}>
      {decorationIcon ? (
        <img src={decorationIcon} alt="" draggable={false} />
      ) : isApplication && nativeUrl ? (
        <img
          src={nativeUrl}
          alt=""
          draggable={false}
          onError={() => setNative({ key: iconKey, url: null })}
        />
      ) : isApplication && e.path.includes('://') && !cachedPresentation(e) ? (
        <span aria-hidden="true" />
      ) : (
        <img className={generic} src={visual.url} alt="" draggable={false} />
      )}
      {e.isSymlink && <Link2 className="symlink" size={12} />}
    </span>
  )
}
