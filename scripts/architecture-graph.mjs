export function stronglyConnectedComponents(graph) {
  let nextIndex = 0
  const stack = []
  const onStack = new Set()
  const indexByNode = new Map()
  const lowLinkByNode = new Map()
  const components = []

  function visit(node) {
    indexByNode.set(node, nextIndex)
    lowLinkByNode.set(node, nextIndex)
    nextIndex += 1
    stack.push(node)
    onStack.add(node)

    for (const neighbor of graph.get(node) ?? []) {
      if (!graph.has(neighbor)) continue
      if (!indexByNode.has(neighbor)) {
        visit(neighbor)
        lowLinkByNode.set(node, Math.min(lowLinkByNode.get(node), lowLinkByNode.get(neighbor)))
      } else if (onStack.has(neighbor)) {
        lowLinkByNode.set(node, Math.min(lowLinkByNode.get(node), indexByNode.get(neighbor)))
      }
    }

    if (lowLinkByNode.get(node) !== indexByNode.get(node)) return
    const component = []
    let member
    do {
      member = stack.pop()
      onStack.delete(member)
      component.push(member)
    } while (member !== node)

    if (component.length > 1 || (graph.get(node) ?? new Set()).has(node)) {
      components.push(component.sort())
    }
  }

  for (const node of graph.keys()) {
    if (!indexByNode.has(node)) visit(node)
  }

  return components.sort((left, right) => left[0].localeCompare(right[0]))
}

export const CRATE_DEPENDENCY_LAYERS = Object.freeze({
  contracts: new Set(['contracts']),
  sdk: new Set(['contracts', 'sdk']),
  foundation: new Set(['contracts', 'foundation']),
  platform: new Set(['contracts', 'foundation', 'platform']),
  builtins: new Set(['contracts', 'foundation', 'platform', 'builtins']),
  app: new Set(['app', 'builtins', 'contracts', 'foundation', 'platform', 'sdk']),
})

export function crateDependencyViolation(sourceLayer, targetLayer) {
  return !CRATE_DEPENDENCY_LAYERS[sourceLayer]?.has(targetLayer)
}

export function coreModuleBoundaryViolations(filePath, source) {
  const normalizedPath = filePath.replaceAll('\\', '/')
  if (!/^src-tauri\/src\/core\/(?:fs|transfers)\//.test(normalizedPath)) return []
  if (
    /(?:^|\/)tests?\//.test(normalizedPath) ||
    /(?:^|\/)(?:tests?|[^/]+_tests?)\.rs$/.test(normalizedPath)
  )
    return []

  return source
    .split('\n')
    .flatMap((line, index) =>
      /(?:crate::)?core::operations\b/.test(line)
        ? [`${normalizedPath}:${index + 1} depends on the operations façade`]
        : [],
    )
}
