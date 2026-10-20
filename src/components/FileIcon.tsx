import { useEffect, useRef, useState } from 'react'
import { Link2 } from 'lucide-react'
import { call } from '@/lib/api'
import { kind } from '@/lib/files'
import { fileVisual } from '@/lib/fileIcons'
import type { Entry } from '@/lib/types'

const cache = new Map<string, Promise<string | null>>()
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
  const isApplication = kind(e) === '应用程序'
  const [native, setNative] = useState<string | null>(null)
  const ref = useRef<HTMLSpanElement>(null)
  const pixels = Math.min(
    512,
    Math.max(32, Math.ceil((size * (window.devicePixelRatio || 1)) / 32) * 32),
  )
  useEffect(() => {
    setNative(null)
    if (!isApplication || !ref.current) return
    let active = true
    const stop = whenVisible(ref.current, () => {
      const key = `${e.path}:${e.modified}:${pixels}`
      if (!cache.has(key)) {
        if (cache.size > 600) cache.clear()
        cache.set(
          key,
          call<string>('native_icon', { path: e.path, size: pixels }).catch(() => null),
        )
      }
      void cache.get(key)!.then((url) => {
        if (active) setNative(url)
      })
    })
    return () => {
      active = false
      stop()
    }
  }, [e.path, e.modified, isApplication, pixels])

  const visual = fileVisual(e)
  const generic = visual.generic ? (visual.folder ? 'generic-folder' : 'generic-file') : ''
  return (
    <span ref={ref} className="file-icon" style={{ width: size, height: size }}>
      {isApplication && native ? (
        <img src={native} alt="" draggable={false} onError={() => setNative(null)} />
      ) : (
        <img className={generic} src={visual.url} alt="" draggable={false} />
      )}
      {e.isSymlink && <Link2 className="symlink" size={12} />}
    </span>
  )
}
