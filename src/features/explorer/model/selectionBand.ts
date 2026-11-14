export interface SelectionBandRect {
  left: number
  top: number
  right: number
  bottom: number
  width: number
  height: number
}

export interface RectBounds {
  left: number
  top: number
  right: number
  bottom: number
}

export function selectionBandRectangle(
  start: { x: number; y: number },
  current: { x: number; y: number },
): SelectionBandRect {
  const left = Math.min(start.x, current.x)
  const top = Math.min(start.y, current.y)
  const width = Math.abs(current.x - start.x)
  const height = Math.abs(current.y - start.y)
  return { left, top, right: left + width, bottom: top + height, width, height }
}

export function isSelectionBandDragged(rect: SelectionBandRect): boolean {
  return rect.width > 3 || rect.height > 3
}

export function rectanglesOverlap(first: RectBounds, second: RectBounds): boolean {
  return (
    first.left < second.right &&
    first.right > second.left &&
    first.top < second.bottom &&
    first.bottom > second.top
  )
}

export function mergeBandSelection(base: string[], picked: string[], additive: boolean): string[] {
  return additive ? [...new Set([...base, ...picked])] : picked
}
