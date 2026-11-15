import { describe, expect, it } from 'vitest'
import { mergePaneSelections, selectedEntriesAcrossPanes } from './paneSelection'

describe('mergePaneSelections', () => {
  it('keeps the single-pane selection unchanged', () => {
    const left = ['left/a', 'left/b']

    expect(mergePaneSelections(false, left, ['right/c'])).toBe(left)
  })

  it('merges split-pane paths once, preserving pane order', () => {
    expect(mergePaneSelections(true, ['a', 'shared'], ['shared', 'b'])).toEqual([
      'a',
      'shared',
      'b',
    ])
  })
})

describe('selectedEntriesAcrossPanes', () => {
  it('keeps the current pane entries when split view is closed', () => {
    const chosen = [{ path: 'left/a', name: 'a' }]

    expect(
      selectedEntriesAcrossPanes(false, [], [{ path: 'right/b', name: 'b' }], chosen, ['left/a']),
    ).toBe(chosen)
  })

  it('reconciles selected paths from both panes and uses the later entry for duplicate paths', () => {
    const left = [
      { path: 'left/a', name: 'a' },
      { path: 'shared/item', name: 'left copy' },
    ]
    const right = [
      { path: 'right/b', name: 'b' },
      { path: 'shared/item', name: 'right copy' },
    ]

    expect(
      selectedEntriesAcrossPanes(
        true,
        left,
        right,
        [],
        ['left/a', 'right/b', 'shared/item', 'missing'],
      ),
    ).toEqual([
      { path: 'left/a', name: 'a' },
      { path: 'shared/item', name: 'right copy' },
      { path: 'right/b', name: 'b' },
    ])
  })
})
