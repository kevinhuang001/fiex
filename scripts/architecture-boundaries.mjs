export function featureDependencyViolation(sourcePath, targetPath) {
  const source = sourcePath.replaceAll('\\', '/')
  const target = targetPath.replaceAll('\\', '/')
  if (!source.startsWith('src/features/')) return false
  return ['src/components', 'src/hooks'].some(
    (layer) => target === layer || target.startsWith(`${layer}/`),
  )
}
