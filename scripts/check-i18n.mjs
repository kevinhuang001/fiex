import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'
const root = fileURLToPath(new URL('../', import.meta.url))
function catalog(name) {
  const file = path.join(root, 'src/i18n/messages', name)
  const text = fs.readFileSync(file, 'utf8')
  const source = ts.parseJsonText(file, text)
  const visit = (node) => {
    if (ts.isObjectLiteralExpression(node)) {
      const names = new Set()
      for (const property of node.properties) {
        const key = property.name?.text
        if (names.has(key)) throw new Error(`${file}: duplicate translation: ${key}`)
        names.add(key)
      }
    }
    ts.forEachChild(node, visit)
  }
  visit(source)
  return JSON.parse(text)
}
const en = catalog('en.json')
const traditional = catalog('zh-TW.json')
const slots = (value) =>
  [...value.matchAll(/\{([\w]*)(?::[^}]*)?\}/g)]
    .map((m) => m[1])
    .sort()
    .join(',')
const errors = []
for (const key of new Set([...Object.keys(en), ...Object.keys(traditional)])) {
  const english = en[key]
  for (const [locale, value] of [
    ['en', english],
    ['zh-TW', traditional[key]],
  ]) {
    if (typeof value !== 'string' || slots(value) !== slots(key))
      errors.push(`${locale}: missing message or mismatched placeholders: ${key}`)
  }
}
function scan(dir) {
  for (const name of fs.readdirSync(dir)) {
    const file = path.join(dir, name)
    if (fs.statSync(file).isDirectory()) {
      if (!['demo', 'i18n'].includes(name)) scan(file)
    } else if (/\.tsx?$/.test(name) && !name.includes('.test.')) {
      const source = ts.createSourceFile(
        file,
        fs.readFileSync(file, 'utf8'),
        ts.ScriptTarget.Latest,
        true,
        name.endsWith('tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
      )
      const visit = (node) => {
        if (ts.isJsxText(node) && /[\p{Script=Han}]/u.test(node.text))
          errors.push(`${file}: hardcoded JSX text: ${node.text.trim()}`)
        if (
          ts.isJsxAttribute(node) &&
          ['title', 'placeholder', 'aria-label'].includes(node.name.getText(source)) &&
          node.initializer &&
          ts.isStringLiteral(node.initializer) &&
          /[\p{Script=Han}]/u.test(node.initializer.text)
        )
          errors.push(`${file}: hardcoded UI attribute: ${node.initializer.text}`)
        if (
          ts.isCallExpression(node) &&
          node.expression.getText(source) === 't' &&
          node.arguments[0] &&
          ts.isStringLiteral(node.arguments[0]) &&
          !Object.hasOwn(en, node.arguments[0].text)
        )
          errors.push(`${file}: missing message: ${node.arguments[0].text}`)
        ts.forEachChild(node, visit)
      }
      visit(source)
    }
  }
}
scan(path.join(root, 'src'))
if (errors.length) {
  console.error(errors.join('\n'))
  process.exitCode = 1
} else
  console.log(
    `i18n: ${Object.keys(en).length} messages; both translations and placeholders verified`,
  )
