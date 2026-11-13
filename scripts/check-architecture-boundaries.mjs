#!/usr/bin/env node
import { readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import ts from 'typescript'
import { featureDependencyViolation } from './architecture-boundaries.mjs'

const ROOT = process.cwd()
const SOURCE_ROOT = path.join(ROOT, 'src')
const FEATURE_ROOT = path.join(SOURCE_ROOT, 'features')
const SOURCE_EXTENSIONS = new Set(['.ts', '.tsx'])
const violations = []

function sourceFiles(directory, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name)
    if (entry.isDirectory()) sourceFiles(file, files)
    else if (SOURCE_EXTENSIONS.has(path.extname(file))) files.push(file)
  }
  return files
}

function importSpecifiers(sourceFile) {
  const specifiers = []
  function visit(node) {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) {
      if (ts.isStringLiteral(node.moduleSpecifier)) specifiers.push(node.moduleSpecifier.text)
    } else if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        (ts.isIdentifier(node.expression) && node.expression.text === 'require')) &&
      node.arguments.length === 1 &&
      ts.isStringLiteral(node.arguments[0])
    ) {
      specifiers.push(node.arguments[0].text)
    } else if (
      ts.isImportTypeNode(node) &&
      ts.isLiteralTypeNode(node.argument) &&
      ts.isStringLiteral(node.argument.literal)
    ) {
      specifiers.push(node.argument.literal.text)
    }
    ts.forEachChild(node, visit)
  }
  visit(sourceFile)
  return specifiers
}

function resolveSourceImport(sourcePath, specifier) {
  if (specifier.startsWith('@/')) return path.resolve(SOURCE_ROOT, specifier.slice(2))
  if (specifier.startsWith('.')) return path.resolve(path.dirname(sourcePath), specifier)
  return null
}

for (const file of sourceFiles(FEATURE_ROOT)) {
  const sourcePath = path.relative(ROOT, file).replaceAll('\\', '/')
  const sourceFile = ts.createSourceFile(
    file,
    readFileSync(file, 'utf8'),
    ts.ScriptTarget.Latest,
    true,
    file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  )
  for (const specifier of importSpecifiers(sourceFile)) {
    const target = resolveSourceImport(file, specifier)
    if (!target) continue
    const targetPath = path.relative(ROOT, target).replaceAll('\\', '/')
    if (featureDependencyViolation(sourcePath, targetPath)) {
      violations.push(`${sourcePath}: feature code cannot import UI or hook module ${specifier}`)
    }
  }
}

if (violations.length) {
  console.error('Architecture boundary check failed:')
  for (const violation of violations) console.error(`- ${violation}`)
  process.exit(1)
}

console.log(
  `Architecture boundary check passed: ${sourceFiles(FEATURE_ROOT).length} feature files checked.`,
)
