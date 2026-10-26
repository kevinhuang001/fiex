import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'
const root = fileURLToPath(new URL('../', import.meta.url))
const en = JSON.parse(fs.readFileSync(path.join(root, 'src/i18n/messages/en.json')))
const traditional = JSON.parse(fs.readFileSync(path.join(root, 'src/i18n/messages/zh-TW.json')))
const slots = (value) =>
  [...value.matchAll(/\{([\w]*)(?::[^}]*)?\}/g)]
    .map((m) => m[1])
    .sort()
    .join(',')
const errors = []
for (const [key, english] of Object.entries(en)) {
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
