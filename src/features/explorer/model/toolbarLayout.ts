export type ToolbarOrderPresentation = {
  drag: { from: number; x: number } | null
  shift(index: number): string | undefined
}

export function orderedToolbarKeys(saved: string[], defaults: string[]): string[] {
  return [
    ...saved.filter((id) => defaults.includes(id)),
    ...defaults.filter((id) => !saved.includes(id)),
  ]
}

export function toolbarItemStyle(
  order: ToolbarOrderPresentation,
  index: number,
): { transform: string; zIndex?: number; position?: 'relative' } | undefined {
  if (order.drag?.from === index) {
    return {
      transform: `translateX(${order.drag.x}px)`,
      zIndex: 2,
      position: 'relative',
    }
  }
  const transform = order.shift(index)
  return transform ? { transform } : undefined
}
