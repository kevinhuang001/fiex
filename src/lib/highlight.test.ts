import { describe, expect, it } from 'vitest'
import { codeLanguage, highlightCode } from './highlight'
describe('code highlighting', () => {
  it('maps common source extensions', () => {
    expect(codeLanguage('rs')).toBe('rust')
    expect(codeLanguage('tsx')).toBe('typescript')
    expect(codeLanguage('yml')).toBe('yaml')
    expect(codeLanguage('txt')).toBeNull()
  })
  it('highlights Rust and escapes source HTML', () => {
    const html = highlightCode('fn main() { let x = "<script>"; }', 'rs')
    expect(html).toContain('hljs-keyword')
    expect(html).toContain('&lt;script&gt;')
    expect(html).not.toContain('<script>')
  })
  it('leaves unknown text plain', () => {
    expect(highlightCode('hello', 'txt')).toBeNull()
  })
})
