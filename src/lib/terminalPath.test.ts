import { describe, expect, it } from 'vitest'
import { terminalWorkingDirectory } from './terminalPath'

describe('terminal working directory', () => {
  it('uses local paths directly and falls back to home for virtual resources', () => {
    expect(terminalWorkingDirectory('/Users/kevin/Downloads')).toBe('/Users/kevin/Downloads')
    expect(terminalWorkingDirectory('C:\\Users\\Kevin')).toBe('C:\\Users\\Kevin')
    expect(terminalWorkingDirectory('fiex://mount/provider/folder')).toBe('')
    expect(terminalWorkingDirectory('recent://item')).toBe('')
  })
})
