export type ReorderKind = 'section' | 'place'
export type ReorderBox = {
  top: number
  left: number
  width: number
  height: number
  center: number
}
export type ReorderDrag = {
  kind: ReorderKind
  from: number
  to: number
  left: number
  top: number
  width: number
  height: number
  gap: number
  settling: boolean
}

export function moveItem<T>(items: T[], from: number, to: number): T[] {
  const next = [...items]
  const [item] = next.splice(from, 1)
  next.splice(to, 0, item)
  return next
}

export function reorderGap(boxes: ReorderBox[]): number {
  return boxes.length > 1 ? Math.max(0, boxes[1].top - (boxes[0].top + boxes[0].height)) : 0
}

export function reorderTargetIndex(center: number, boxes: ReorderBox[], from: number): number {
  let target = 0
  for (let index = 0; index < boxes.length; index++) {
    if (index !== from && center > boxes[index].center) target++
  }
  return target
}

export function reorderShift(
  drag: ReorderDrag | null,
  kind: ReorderKind,
  index: number,
): string | undefined {
  if (!drag || drag.kind !== kind || index === drag.from) return undefined
  const distance = drag.height + drag.gap
  if (drag.from < drag.to && index > drag.from && index <= drag.to)
    return `translate3d(0, -${distance}px, 0)`
  if (drag.to < drag.from && index >= drag.to && index < drag.from)
    return `translate3d(0, ${distance}px, 0)`
  return undefined
}
