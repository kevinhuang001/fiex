import { describe, expect, it } from 'vitest'
import { featureDependencyViolation } from '../../scripts/architecture-boundaries.mjs'

describe('feature dependency boundaries', () => {
  it('rejects feature dependencies on components and hooks', () => {
    expect(
      featureDependencyViolation('src/features/explorer/model/name.ts', 'src/hooks/useFiles.ts'),
    ).toBe(true)
    expect(featureDependencyViolation('src/features/a.ts', 'src/hooks')).toBe(true)
    expect(
      featureDependencyViolation('src/features/explorer/model/name.ts', 'src/components/File.tsx'),
    ).toBe(true)
  })

  it('allows shared dependencies and imports into features', () => {
    expect(
      featureDependencyViolation('src/features/explorer/model/name.ts', 'src/lib/types.ts'),
    ).toBe(false)
    expect(
      featureDependencyViolation(
        'src/components/Browser.tsx',
        'src/features/explorer/model/name.ts',
      ),
    ).toBe(false)
  })
})
