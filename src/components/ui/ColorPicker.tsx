import { useEffect, useRef, useState } from 'react'

type Hsv = { h: number; s: number; v: number }

const clamp = (value: number, min = 0, max = 1) => Math.min(max, Math.max(min, value))

function parseHex(value: string): [number, number, number] | null {
  const match = /^#?([0-9a-f]{6})$/i.exec(value.trim())
  if (!match) return null
  return [
    parseInt(match[1].slice(0, 2), 16),
    parseInt(match[1].slice(2, 4), 16),
    parseInt(match[1].slice(4, 6), 16),
  ]
}

function rgbToHsv(red: number, green: number, blue: number): Hsv {
  const r = red / 255
  const g = green / 255
  const b = blue / 255
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const delta = max - min
  let h = 0
  if (delta > 0) {
    if (max === r) h = ((g - b) / delta) % 6
    else if (max === g) h = (b - r) / delta + 2
    else h = (r - g) / delta + 4
    h *= 60
    if (h < 0) h += 360
  }
  return { h, s: max === 0 ? 0 : delta / max, v: max }
}

function hsvToHex({ h, s, v }: Hsv): string {
  const chroma = v * s
  const section = (h / 60) % 6
  const x = chroma * (1 - Math.abs((section % 2) - 1))
  const [r, g, b] =
    section < 1
      ? [chroma, x, 0]
      : section < 2
        ? [x, chroma, 0]
        : section < 3
          ? [0, chroma, x]
          : section < 4
            ? [0, x, chroma]
            : section < 5
              ? [x, 0, chroma]
              : [chroma, 0, x]
  const m = v - chroma
  return (
    '#' +
    [r, g, b]
      .map((channel) =>
        Math.round((channel + m) * 255)
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  )
}

/// Inline HSV picker used by the tag creator. The value is kept in sync when a
/// preset swatch is clicked, while dragging stays local to the component.
export function ColorPicker({
  value,
  onChange,
  label = '颜色',
}: {
  value: string
  onChange: (hex: string) => void
  label?: string
}) {
  const initial = parseHex(value) || [0, 122, 255]
  const [hsv, setHsv] = useState<Hsv>(() => rgbToHsv(initial[0], initial[1], initial[2]))
  const [draft, setDraft] = useState(value.replace('#', '').toUpperCase())
  const emitted = useRef(value.toLowerCase())
  const map = useRef<HTMLDivElement>(null)
  const hue = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (value.toLowerCase() === emitted.current) return
    const rgb = parseHex(value)
    if (!rgb) return
    emitted.current = value.toLowerCase()
    setHsv(rgbToHsv(rgb[0], rgb[1], rgb[2]))
    setDraft(value.replace('#', '').toUpperCase())
  }, [value])

  const emit = (next: Hsv) => {
    setHsv(next)
    const hex = hsvToHex(next)
    emitted.current = hex
    setDraft(hex.replace('#', '').toUpperCase())
    onChange(hex)
  }

  const pick = (event: React.PointerEvent<HTMLDivElement>) => {
    const rect = map.current?.getBoundingClientRect()
    if (!rect) return
    const s = clamp((event.clientX - rect.left) / rect.width)
    const v = 1 - clamp((event.clientY - rect.top) / rect.height)
    emit({ ...hsv, s, v })
  }

  const pickHue = (event: React.PointerEvent<HTMLDivElement>) => {
    const rect = hue.current?.getBoundingClientRect()
    if (!rect) return
    emit({ ...hsv, h: clamp((event.clientX - rect.left) / rect.width) * 360 })
  }

  const current = hsvToHex(hsv)
  return (
    <div className="color-picker">
      <div
        ref={map}
        className="color-picker-map"
        role="group"
        aria-label={`${label}饱和度和明度`}
        style={{
          background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(${hsv.h} 100% 50%))`,
        }}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId)
          pick(event)
        }}
        onPointerMove={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId)) pick(event)
        }}
      >
        <i
          className="color-picker-thumb"
          style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%` }}
        />
      </div>
      <div
        ref={hue}
        className="color-picker-hue"
        role="group"
        aria-label={`${label}色相`}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId)
          pickHue(event)
        }}
        onPointerMove={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId)) pickHue(event)
        }}
      >
        <i className="color-picker-thumb" style={{ left: `${(hsv.h / 360) * 100}%` }} />
      </div>
      <label className="color-picker-hex">
        <span className="color-picker-preview" style={{ background: current }} />
        <span className="color-picker-hash">#</span>
        <input
          aria-label={`${label}十六进制值`}
          value={draft}
          maxLength={6}
          spellCheck={false}
          onChange={(event) => {
            const next = event.target.value.replace(/[^0-9a-f]/gi, '').toUpperCase()
            setDraft(next)
            if (next.length === 6) onChange(`#${next.toLowerCase()}`)
          }}
          onBlur={() => setDraft(current.replace('#', '').toUpperCase())}
        />
      </label>
    </div>
  )
}
