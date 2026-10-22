import hljs from 'highlight.js/lib/core'
import bash from 'highlight.js/lib/languages/bash'
import c from 'highlight.js/lib/languages/c'
import cpp from 'highlight.js/lib/languages/cpp'
import csharp from 'highlight.js/lib/languages/csharp'
import css from 'highlight.js/lib/languages/css'
import diff from 'highlight.js/lib/languages/diff'
import dockerfile from 'highlight.js/lib/languages/dockerfile'
import go from 'highlight.js/lib/languages/go'
import ini from 'highlight.js/lib/languages/ini'
import java from 'highlight.js/lib/languages/java'
import javascript from 'highlight.js/lib/languages/javascript'
import json from 'highlight.js/lib/languages/json'
import makefile from 'highlight.js/lib/languages/makefile'
import markdown from 'highlight.js/lib/languages/markdown'
import python from 'highlight.js/lib/languages/python'
import rust from 'highlight.js/lib/languages/rust'
import sql from 'highlight.js/lib/languages/sql'
import typescript from 'highlight.js/lib/languages/typescript'
import xml from 'highlight.js/lib/languages/xml'
import yaml from 'highlight.js/lib/languages/yaml'
const languages = {
  bash,
  c,
  cpp,
  csharp,
  css,
  diff,
  dockerfile,
  go,
  ini,
  java,
  javascript,
  json,
  makefile,
  markdown,
  python,
  rust,
  sql,
  typescript,
  xml,
  yaml,
}
for (const [name, language] of Object.entries(languages)) hljs.registerLanguage(name, language)
const aliases: Record<string, string> = {
  ts: 'typescript',
  tsx: 'typescript',
  mts: 'typescript',
  cts: 'typescript',
  js: 'javascript',
  jsx: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  rs: 'rust',
  py: 'python',
  json: 'json',
  jsonc: 'json',
  sh: 'bash',
  zsh: 'bash',
  fish: 'bash',
  scss: 'css',
  less: 'css',
  html: 'xml',
  htm: 'xml',
  vue: 'xml',
  svelte: 'xml',
  svg: 'xml',
  md: 'markdown',
  yml: 'yaml',
  toml: 'ini',
  lock: 'ini',
  h: 'c',
  cc: 'cpp',
  cxx: 'cpp',
  hpp: 'cpp',
  hh: 'cpp',
  cs: 'csharp',
  conf: 'ini',
  cfg: 'ini',
  patch: 'diff',
}
export function codeLanguage(extension: string, name = ''): string | null {
  if (name.toLowerCase() === 'dockerfile') return 'dockerfile'
  if (name.toLowerCase() === 'makefile') return 'makefile'
  return aliases[extension.toLowerCase()] || null
}
export function highlightCode(code: string, extension: string, name = ''): string | null {
  const language = codeLanguage(extension, name)
  if (!language) return null
  try {
    return hljs.highlight(code, { language, ignoreIllegals: true }).value
  } catch {
    return null
  }
}
