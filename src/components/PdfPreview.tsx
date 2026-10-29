import { t } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import {
  getDocument,
  GlobalWorkerOptions,
  type PDFDocumentProxy,
} from 'pdfjs-dist/legacy/build/pdf.mjs'
import worker from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'
import { ChevronLeft, ChevronRight, LoaderCircle } from 'lucide-react'
import { Button } from './ui/button'
GlobalWorkerOptions.workerSrc = worker
export default function PdfPreview({ url }: { url: string }) {
  const [document, setDocument] = useState<PDFDocumentProxy | null>(null)
  const [page, setPage] = useState(1)
  const [error, setError] = useState('')
  const [width, setWidth] = useState(650)
  const [loading, setLoading] = useState(true)
  const canvas = useRef<HTMLCanvasElement>(null)
  const container = useRef<HTMLDivElement>(null)
  useEffect(() => {
    setPage(1)
    setDocument(null)
    setError('')
    setLoading(true)
    const task = getDocument({ url, useSystemFonts: true })
    let active = true
    void task.promise
      .then((doc) => {
        if (active) setDocument(doc)
      })
      .catch((e) => {
        if (active) {
          setError(t('无法预览 PDF：{0}', String(e)))
          setLoading(false)
        }
      })
    return () => {
      active = false
      void task.destroy()
    }
  }, [url])
  useEffect(() => {
    const observer = new ResizeObserver(([entry]) =>
      setWidth(Math.max(200, entry.contentRect.width - 50)),
    )
    if (container.current) observer.observe(container.current)
    return () => observer.disconnect()
  }, [])
  useEffect(() => {
    if (!document || !canvas.current) return
    let active = true
    let cancel: (() => void) | undefined
    setLoading(true)
    void document
      .getPage(page)
      .then((pdf) => {
        if (!active || !canvas.current) return
        const natural = pdf.getViewport({ scale: 1 })
        const scale = Math.min(width / natural.width, 2)
        const ratio = Math.min(devicePixelRatio, 2)
        const viewport = pdf.getViewport({ scale: scale * ratio })
        const el = canvas.current
        el.width = viewport.width
        el.height = viewport.height
        el.style.width = `${viewport.width / ratio}px`
        el.style.height = `${viewport.height / ratio}px`
        const render = pdf.render({ canvas: el, viewport })
        cancel = () => render.cancel()
        return render.promise
      })
      .then(() => {
        if (active) setLoading(false)
      })
      .catch((e) => {
        if (active) {
          setError(String(e))
          setLoading(false)
        }
      })
    return () => {
      active = false
      cancel?.()
    }
  }, [document, page, width])
  return (
    <div className="pdf-preview" ref={container}>
      <div className="pdf-controls">
        <Button
          variant="ghost"
          size="icon"
          title={t('上一页')}
          disabled={page <= 1}
          onClick={() => setPage((p) => p - 1)}
        >
          <ChevronLeft />
        </Button>
        <span>
          {page} / {document?.numPages || '…'}
        </span>
        <Button
          variant="ghost"
          size="icon"
          title={t('下一页')}
          disabled={!document || page >= document.numPages}
          onClick={() => setPage((p) => p + 1)}
        >
          <ChevronRight />
        </Button>
        {loading && <LoaderCircle className="spin" size={13} />}
      </div>
      {error ? (
        <p className="error">
          {error}
          <br />
          {t('可点击下方“打开”使用系统默认应用查看。')}
        </p>
      ) : (
        <canvas ref={canvas} />
      )}
    </div>
  )
}
