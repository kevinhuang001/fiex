interface MenuGroup {
  items: { parentId?: string; kind: string }[]
}

/** Menus made only of separators or child items have nothing to display. */
export function hasTopLevelMenuActions(groups: MenuGroup[]) {
  return groups.some((group) =>
    group.items.some((item) => !item.parentId && item.kind !== 'separator'),
  )
}
