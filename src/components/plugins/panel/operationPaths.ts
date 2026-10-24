/** File chooser results are candidates, not filesystem grants. Grant them only
 * on the later plugin invocation that names them as operation inputs. */
export function operationPaths(params: unknown, candidates: Iterable<string>): string[] {
  const named = new Set<string>()
  const visit = (value: unknown, depth: number) => {
    if (depth > 8) return
    if (typeof value === 'string') named.add(value)
    else if (Array.isArray(value)) value.forEach((item) => visit(item, depth + 1))
    else if (value && typeof value === 'object')
      Object.values(value).forEach((item) => visit(item, depth + 1))
  }
  visit(params, 0)
  return [...new Set(candidates)].filter((path) => named.has(path))
}
